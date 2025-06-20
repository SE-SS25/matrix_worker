DROP TABLE IF EXISTS "user";

CREATE TABLE "user"
(
    id   SERIAL PRIMARY KEY,
    name TEXT NOT NULL
);

INSERT INTO "user" (name)
VALUES ('Leon');

INSERT INTO "user" (name)
VALUES ('Linus');

INSERT INTO "user" (name)
VALUES ('Constantin');

INSERT INTO "user" (name)
VALUES ('Cedric');
