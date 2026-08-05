use std::{
	collections::HashSet,
	path::{Path, PathBuf},
};

use reqwest::{
	Client,
	header::{CONTENT_TYPE, USER_AGENT},
};
use rusqlite::Connection;
use server::ytmusic::{YouTubeMusicClient, player::AudioFormat};

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

		tokio::fs::create_dir(folder.join("audio")).await?;
		tokio::fs::create_dir(folder.join("thumbnail")).await?;

		let db = Connection::open(path)?;

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

               -- youtube_playlist_name text,
               youtube_playlist_id text,
            );
						CREATE TEMP TABLE IF NOT EXISTS incoming_tracks(
							youtube_video_id text NOT NULL PRIMARY KEY,
						);
    				CREATE TABLE IF NOT EXISTS tracks(
   				    id integer PRIMARY KEY,
    					title text NOT NULL,

              audio_path text NOT NULL,
              thumbnail_path text,

              youtube_video_id text NOT NULL UNIQUE,
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

		Ok(Self {
			db,
			folder,
			client: YouTubeMusicClient::new(),
			reqwest_client: Client::new(),
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

		let response = self
			.reqwest_client
			.get(stream_info.url)
			.header(USER_AGENT, &stream_info.user_agent)
			.send()
			.await?;

		tokio::fs::write(&audio_path, response.bytes().await?).await?;

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

		// TODO: is asking the database for which ids don't exist faster or slower?
		// TODO: would that lead to overhead? because tracks are already allocated once

		let already_existing_track_ids = self
			.db
			.prepare(
				"SELECT s.youtube_video_id
				FROM tracks s
				JOIN incoming_tracks t ON t.id = s.youtube_video_id;
			",
			)?
			.query_map((), |row| row.get::<_, String>(0))?
			.filter_map(Result::ok)
			.collect::<HashSet<_>>();

		let mut insert_track_statement = self.db.prepare(
			"
			INSERT INTO tracks (title, audio_path, thumbnail_path, youtube_video_id) VALUES (?1, ?2, ?3, ?4)
			ON CONFLICT (youtube_video_id) DO UPDATE SET
				title = excluded.title,
				audio_path = excluded.audio_path,
				thumbnail_path = excluded.thumbnail_path;
			",
		)?;

		for track in tracks {
			let youtube_video_id = track.id.key();

			if already_existing_track_ids.contains(&*youtube_video_id) {
				continue;
			}

			let (resulting_audio_path, user_agent) =
				self.download_single_track(&*youtube_video_id).await?;

			let thumbnail_path = self
				.download_thumbnail(&*youtube_video_id, track.cover, user_agent)
				.await
				.unwrap_or(None);

			insert_track_statement.execute((
				track.title,
				resulting_audio_path.to_string_lossy().to_string(),
				thumbnail_path.map(|path| path.to_string_lossy().to_string()),
				youtube_video_id,
			))?;
		}

		// clean temporary table to increase performance
		self.db.execute("DELETE FROM incoming_tracks;", [])?;

		Ok(())
	}
}

#[tokio::main]
async fn main() {
	let playlist_id = "OLAK5uy_nFiS1SeXBnJII-kBfpg7kGRB0JeE_tot8";
	let client = YouTubeMusicClient::new();
	let entries = client
		.get_playlist_entries(playlist_id)
		.await
		.expect("failed getting entries of playlist");

	let first_track = entries.first().expect("no first entry");

	let video_id = first_track.id.key();
	let stream = client
		.get_stream(&video_id)
		.await
		.expect("failed getting stream");
	println!("stream: {stream:?}");
	println!("cover: {:?}", first_track.cover)
}
