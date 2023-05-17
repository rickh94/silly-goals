CREATE TYPE goal_behavior AS ENUM ('hide', 'nice', 'mean');
CREATE TYPE deadline_type AS ENUM ('off', 'soft', 'hard');

CREATE TABLE tones (
	id BIGSERIAL PRIMARY KEY,
	name VARCHAR(250) NOT NULL,
	user_id BIGINT NOT NULL,
	global BOOLEAN DEFAULT false NOT NULL,
	stages text[4] NOT NULL,
	greeting VARCHAR(250) NOT NULL,
	unmet_behavior goal_behavior NOT NULL,
	deadline deadline_type DEFAULT 'off' NOT NULL,
	CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id)
);

CREATE INDEX "tones_user_id" ON tones(user_id);

CREATE TABLE groups (
	id BIGSERIAL PRIMARY KEY,
	title VARCHAR(250) NOT NULL,
	description TEXT,
	user_id BIGINT NOT NULL,
	tone_id BIGINT NOT NULL,
	CONSTRAINT fk_user FOREIGN KEY (user_id) references users(id),
	CONSTRAINT fk_tone FOREIGN KEY (tone_id) REFERENCES tones(id)
);

CREATE INDEX "groups_user_id" ON groups(user_id);

CREATE TABLE goals (
	id BIGSERIAL PRIMARY KEY,
	title VARCHAR(250) NOT NULL,
	description TEXT,
	stage SMALLINT NOT NULL,
	group_id BIGINT NOT NULL,
	deadline DATE,
	CONSTRAINT fk_group FOREIGN KEY (group_id) REFERENCES goals(id)
);

CREATE INDEX "goals_group_id" ON goals(group_id);


