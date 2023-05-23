CREATE TABLE tones (
	id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
	name TEXT NOT NULL,
	user_id INTEGER NOT NULL,
	global INTEGER DEFAULT 0 NOT NULL,
	stages TEXT NOT NULL,
	greeting TEXT NOT NULL,
	unmet_behavior TEXT NOT NULL CHECK(unmet_behavior IN ('hide', 'nice', 'mean')),
	deadline TEXT DEFAULT 'off' NOT NULL CHECK (deadline IN ('off', 'soft', 'hard')),
	FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX IF NOT EXISTS "tones_user_id" ON tones(user_id);

CREATE TABLE groups (
	id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
	title TEXT NOT NULL,
	description TEXT,
	user_id INTEGER NOT NULL,
	tone_id INTEGER NOT NULL,
	FOREIGN KEY (user_id) REFERENCES users(id),
	FOREIGN KEY (tone_id) REFERENCES tones(id)
);

CREATE INDEX "groups_user_id" ON groups(user_id);

CREATE TABLE goals (
	id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
	title TEXT NOT NULL,
	description TEXT,
	stage INTEGER NOT NULL,
	group_id INTEGER NOT NULL,
	deadline TEXT,
	FOREIGN KEY (group_id) REFERENCES goals(id)
);

CREATE INDEX "goals_group_id" ON goals(group_id);


