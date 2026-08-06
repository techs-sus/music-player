use std::{
	collections::HashSet,
	io::{BufWriter, Write},
	path::{Path, PathBuf},
};

use clap::Parser;
use reqwest::{
	Client,
	header::{CONTENT_TYPE, RANGE, REFERER, USER_AGENT},
};
use rusqlite::Connection;
use server::ytmusic::YouTubeMusicClient;
use tracing::info;

#[derive(thiserror::Error, Debug)]
enum Error {
	#[error("sqlite error: {0}")]
	Sqlite(#[from] rusqlite::Error),

	#[error("reqwest error: {0}")]
	Reqwest(#[from] reqwest::Error),

	#[error("tokio io error: {0}")]
	Io(#[from] tokio::io::Error),

	#[error("sqlite schema.application_id != MAGIC, close error: {0:?}")]
	NotOurMusicDatabase(Option<rusqlite::Error>),

	#[error("there is no upstream playlist for this playlist")]
	NoUpstreamPlaylist,

	#[error("the playlist has no parent folder for its tracks and thumbnails")]
	PlaylistHasNoParentFolder,

	#[error("failed getting playlist entries from kopuz: {0}")]
	UpstreamGettingPlaylistEntries(String),

	#[error("failed getting a track's stream info from kopuz: {0}")]
	UpstreamGettingStreamInfo(String),
}

/// https://sqlite.org/pragma.html#pragma_application_id
pub const SQLITE_APPLICATION_ID: u32 = 0x7D8A4B83;

// folder will be structured like so:
// - audio
// - thumbnail
// - playlist.db
/// A playlist on disk.
struct Playlist {
	name: String,
	folder: PathBuf,
	db: Connection,
	client: YouTubeMusicClient,
	reqwest_client: reqwest::Client,
}

impl Playlist {
	async fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
		let Some(folder) = path.as_ref().parent().map(Path::to_path_buf) else {
			return Err(Error::PlaylistHasNoParentFolder);
		};

		let folder = tokio::fs::canonicalize(folder).await?;

		let _ = tokio::fs::create_dir(folder.join("audio")).await;
		let _ = tokio::fs::create_dir(folder.join("thumbnail")).await;

		let db = Connection::open(&path)?;

		// https://sqlite.org/pragma.html#pragma_application_id
		let application_id = db.pragma_query_value(
			Some(rusqlite::DatabaseName::Main),
			"application_id",
			|row| row.get::<_, u32>(0),
		)?;

		match application_id {
			0 => {
				// mark the database as ours
				db.pragma_update(
					Some(rusqlite::DatabaseName::Main),
					"application_id",
					SQLITE_APPLICATION_ID,
				)?;

				// first version = 0
				db.pragma_update(Some(rusqlite::DatabaseName::Main), "user_version", 0)?;

				// everything in schema version 0
				db.execute_batch(
					"
    				BEGIN;
    				CREATE TABLE IF NOT EXISTS playlist_metadata(
               singleton_key integer primary key check (singleton_key = 1),

               youtube_playlist_id text
            );	
    				CREATE TABLE IF NOT EXISTS tracks(
   				    id integer PRIMARY KEY,
    					title text NOT NULL,

              audio_path text NOT NULL,
              thumbnail_path text,

              youtube_video_id text NOT NULL UNIQUE,
							position integer NOT NULL UNIQUE
    				);
    				COMMIT;
				",
				)?;
			}

			SQLITE_APPLICATION_ID => {
				// no migrations, so don't do anything
			}

			// another application set a unique application_id, close and error so we don't nuke any data
			_ => {
				// close and throw the error, too much work to close over and over again
				// to be safe we could open as read only, do all checks etc, then reopen as read+write
				db.close()
					.map_err(|(_, e)| Error::NotOurMusicDatabase(Some(e)))?;

				return Err(Error::NotOurMusicDatabase(None));
			}
		}

		db.execute(
			"CREATE TEMP TABLE IF NOT EXISTS incoming_tracks(
				youtube_video_id text NOT NULL PRIMARY KEY
			);",
			(),
		)?;

		Ok(Self {
			db,
			folder,
			client: YouTubeMusicClient::new(),
			reqwest_client: Client::new(),
			name: path
				.as_ref()
				.with_extension("")
				.file_name()
				.expect("playlist should have name")
				.to_string_lossy()
				.to_string(),
		})
	}

	/// Downloads the track's thumbnail and gives back the path to the thumbnail with the proper
	/// extension.
	async fn download_thumbnail(
		&self,
		video_id: &str,
		cover_url: Option<String>,
		user_agent: String,
	) -> Result<Option<PathBuf>, Error> {
		let Some(cover_url) = cover_url else {
			return Ok(None);
		};

		let response = self
			.reqwest_client
			.get(cover_url)
			.header(USER_AGENT, user_agent)
			.header(REFERER, "https://music.youtube.com/")
			.send()
			.await?;

		match response
			.headers()
			.get(CONTENT_TYPE)
			.map(|value| value.to_str())
		{
			None | Some(Err(..)) => Ok(None),
			Some(Ok(content_type)) => {
				let Some(extension) = mime2ext::mime2ext(content_type) else {
					return Ok(None);
				};

				let final_path = self
					.folder
					.join("thumbnail")
					.join(video_id)
					.with_extension(extension)
					.to_path_buf();

				tokio::fs::write(&final_path, response.bytes().await?).await?;

				return Ok(Some(final_path));
			}
		}
	}

	/// Downloads the track and gives back the audio_base_path with the proper extension and the user
	/// agent used to download it.
	#[tracing::instrument(skip(self))]
	async fn download_single_track(&self, video_id: &str) -> Result<(PathBuf, String), Error> {
		let stream_info = self
			.client
			.get_stream(video_id)
			.await
			.map_err(Error::UpstreamGettingStreamInfo)?;

		let audio_path = self
			.folder
			.join("audio")
			.join(video_id)
			.with_extension(stream_info.format.extension());

		// if the file was already downloaded, don't refetch it
		match tokio::fs::try_exists(&audio_path).await {
			Ok(true) => {}

			_ => {
				let response = self
					.reqwest_client
					.get(&stream_info.url)
					.header(USER_AGENT, &stream_info.user_agent)
					.header(REFERER, "https://music.youtube.com/")
					// this header is REQUIRED for near instant downloads
					.header(RANGE, "bytes=0-")
					.send()
					.await?;

				let start = std::time::Instant::now();
				tokio::fs::write(&audio_path, response.bytes().await?).await?;
				info!(
					"downloaded track in {:#?}",
					std::time::Instant::now() - start
				);
			}
		};

		Ok((audio_path, stream_info.user_agent))
	}

	/// Sync from YouTube Music.
	///
	/// # Errors
	/// - [`Error::NoUpstreamPlaylist`] if the playlist has no upstream target to sync from.
	/// - [`Error::UpstreamFailedGettingPlaylistEntries`]
	async fn sync_from_youtube(&mut self) -> Result<(), Error> {
		let Some(playlist_id) = self.db.query_row(
			"SELECT youtube_playlist_id from playlist_metadata",
			[],
			|row| row.get::<_, Option<String>>(0),
		)?
		else {
			return Err(Error::NoUpstreamPlaylist);
		};

		let tracks = self
			.client
			.get_playlist_entries(&playlist_id)
			.await
			.map_err(Error::UpstreamGettingPlaylistEntries)?;

		// put all incoming ids into incoming_tracks
		{
			let tx = self.db.transaction()?;
			let mut prepared =
				tx.prepare("INSERT OR IGNORE INTO incoming_tracks (youtube_video_id) VALUES (?1)")?;

			for track in &tracks {
				prepared.execute([track.id.key()])?;
			}

			drop(prepared);
			tx.commit()?;
		}

		let already_existing_track_ids = self
			.db
			.prepare(
				"SELECT s.youtube_video_id
				FROM tracks s
				JOIN incoming_tracks t ON t.youtube_video_id = s.youtube_video_id;
			",
			)?
			.query_map((), |row| row.get::<_, String>(0))?
			.filter_map(Result::ok)
			.collect::<HashSet<_>>();

		// clean temporary table to increase performance
		self.db.execute("DELETE FROM incoming_tracks;", [])?;

		let mut insert_track_statement = self.db.prepare(
			"
			INSERT INTO tracks (title, audio_path, thumbnail_path, position, youtube_video_id) VALUES (?1, ?2, ?3, ?4, ?5)
			ON CONFLICT (youtube_video_id) DO UPDATE SET
				title = excluded.title,
				audio_path = excluded.audio_path,
				thumbnail_path = excluded.thumbnail_path;
				position = excluded.position;
			",
		)?;

		let mut ensure_track_has_updated_position = self
			.db
			.prepare("UPDATE tracks SET position = ?1 WHERE youtube_video_id = ?2")?;

		for (playlist_ordered_position, track) in tracks.into_iter().enumerate() {
			let youtube_video_id = track.id.key();

			if already_existing_track_ids.contains(&*youtube_video_id) {
				// update track position as it may have changed in the upstream
				info!(
					"track id {} already exists, not fetching from youtube",
					&youtube_video_id
				);

				ensure_track_has_updated_position
					.execute((playlist_ordered_position, &youtube_video_id))?;
				continue;
			}

			let (resulting_audio_path, user_agent) =
				self.download_single_track(&*youtube_video_id).await?;

			let thumbnail_path = self
				.download_thumbnail(&*youtube_video_id, track.cover, user_agent)
				.await
				.unwrap_or(None);

			// all database paths must be relative for portability
			let resulting_audio_path = pathdiff::diff_paths(resulting_audio_path, &self.folder).unwrap();
			let thumbnail_path = thumbnail_path
				.map(|path| pathdiff::diff_paths(path, &self.folder))
				.unwrap();

			insert_track_statement.execute((
				track.title,
				resulting_audio_path.to_string_lossy().to_string(),
				thumbnail_path.map(|path| path.to_string_lossy().to_string()),
				playlist_ordered_position,
				youtube_video_id,
			))?;
		}

		Ok(())
	}

	fn write_playlist_to_m3a(&self) -> Result<(), Error> {
		struct RegularTrack {
			path: PathBuf,
			title: String,
		}

		let mut prepared = self
			.db
			.prepare("SELECT audio_path, title FROM tracks ORDER BY position ASC")?;

		let tracks = prepared.query_map((), |row| {
			Ok(RegularTrack {
				path: self.folder.join(row.get::<_, String>(0)?),
				title: row.get::<_, String>(1)?,
			})
		})?;

		let mut buf = BufWriter::new(std::fs::File::create(
			self.folder.join(&self.name).with_extension("m3a"),
		)?);

		write!(buf, "#EXTM3U\n")?;

		for track in tracks.into_iter().filter_map(Result::ok) {
			write!(buf, "#EXTINF:0,{}\n", track.title)?;
			write!(buf, "{}\n", track.path.to_string_lossy())?;
		}

		buf.flush()?;

		Ok(())
	}

	fn set_upstream(&self, youtube_playlist_id: &str) -> Result<(), Error> {
		self.db.execute(
			"INSERT INTO playlist_metadata (singleton_key, youtube_playlist_id) VALUES (1, ?1) ON CONFLICT(singleton_key) DO UPDATE SET youtube_playlist_id = excluded.youtube_playlist_id",
			[youtube_playlist_id],
		)?;

		Ok(())
	}
}

#[derive(clap::Args)]
struct PlaylistOptions {
	/// The sqlite database file used to sync.
	#[arg(short = 'p', long = "playlist")]
	playlist_db_path: PathBuf,
}

#[derive(clap::Subcommand)]
enum Command {
	/// Initializes a playlist file.
	Init {
		#[clap(flatten)]
		options: PlaylistOptions,

		/// The upstream playlist to init this local one.
		#[arg(short = 'u', long = "upstream")]
		youtube_upstream_playlist: String,
	},

	/// Syncs music from a playlist file and produces an m3u8 file.
	Sync {
		#[clap(flatten)]
		options: PlaylistOptions,
	},

	/// Writes a playlist's m3a file.
	WriteToM3a {
		#[clap(flatten)]
		options: PlaylistOptions,
	},
}

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
	#[command(subcommand)]
	command: Command,
}

#[tokio::main]
async fn main() {
	tracing_subscriber::fmt()
		.compact()
		.with_target(false)
		.without_time()
		.with_level(true)
		.init();
	let args = Args::parse();
	match args.command {
		Command::Init {
			options,
			youtube_upstream_playlist,
		} => {
			let playlist = Playlist::from_path(options.playlist_db_path)
				.await
				.expect("failed opening playlist");

			playlist
				.set_upstream(&youtube_upstream_playlist)
				.expect("failed setting playlist upstream");

			info!("successfully set playlist upstream to {youtube_upstream_playlist}");
		}
		Command::Sync { options } => {
			let mut playlist = Playlist::from_path(options.playlist_db_path)
				.await
				.expect("failed opening playlist");

			playlist
				.sync_from_youtube()
				.await
				.expect("failed syncing from youtube");
			info!("successfully synced from youtube");
		}
		Command::WriteToM3a { options } => {
			let playlist = Playlist::from_path(options.playlist_db_path)
				.await
				.expect("failed opening playlist");

			playlist
				.write_playlist_to_m3a()
				.expect("failed writing playlist to m3a");
			info!("successfully wrote playlist m3a");
		}
	};
}
