CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE TABLE IF NOT EXISTS users(
  id BIGSERIAL PRIMARY KEY,
  userid UUID DEFAULT gen_random_uuid() NOT NULL,
  name VARCHAR(250),
  email VARCHAR(250) UNIQUE NOT NULL
);

CREATE TABLE IF NOT EXISTS webauthn_credentials(
  id UUID DEFAULT gen_random_uuid() PRIMARY KEY NOT NULL,
  passkey TEXT NOT NULL,
  user_id BIGINT NOT NULL,
  CONSTRAINT fk_user FOREIGN KEY (user_id) REFERENCES users(id)
);
