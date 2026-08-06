#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("sqlite error: {0}")]
	Sqlite(#[from] rusqlite::Error),

	#[error("reqwest error: {0}")]
	Reqwest(#[from] reqwest::Error),

	#[error("tokio io error: {0}")]
	Io(#[from] tokio::io::Error),

	#[error("lofty error: {0}")]
	Lofty(#[from] lofty::error::LoftyError),

	#[error("sqlite schema.application_id != MAGIC, close error: {0:?}")]
	NotOurMusicDatabase(Option<rusqlite::Error>),

	#[error("there is no upstream playlist for this playlist")]
	NoUpstreamPlaylist,

	#[error("failed to find or create a primary tag")]
	NoPrimaryTag,

	#[error("the playlist has no parent folder for its tracks and thumbnails")]
	PlaylistHasNoParentFolder,

	#[error("failed getting playlist entries from kopuz: {0}")]
	UpstreamGettingPlaylistEntries(String),

	#[error("failed getting a track's stream info from kopuz: {0}")]
	UpstreamGettingStreamInfo(String),

	#[error("failed to remux a track's webm to its m4a counterpart with ffmpeg: {0}")]
	FailedToRemuxAsM4a(std::io::Error),
}
