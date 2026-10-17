PRAGMA foreign_keys=OFF;
BEGIN TRANSACTION;
CREATE TABLE IF NOT EXISTS "migrations" ("id" integer not null primary key, "name" text not null unique, "applied_at" integer default (unixepoch()) not null) strict;
COMMIT;
