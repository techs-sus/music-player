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
