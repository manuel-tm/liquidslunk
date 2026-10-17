//! Credits with free-text roles. A credit is a line: a role as printed,
//! and its artists in order. `credit_roles` goes away, and the old credits
//! are dropped with it rather than carried over. `down` brings back the
//! baseline's tables, empty except for its starter roles.

use rusqlite::Transaction;

use super::run;
use crate::Result;

pub fn up(tx: &Transaction) -> Result<()> {
    run(tx, UP)
}

pub fn down(tx: &Transaction) -> Result<()> {
    run(tx, DOWN)
}

const UP: &[&str] = &[
    r#"drop table "release_credits""#,
    r#"drop table "track_credits""#,
    r#"drop table "credit_roles""#,
    r#"create table "release_credits" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "role" text not null check ("role" <> '' AND "role" = trim("role")) COLLATE NOCASE, "position" integer not null check (position >= 0), "first_artist_position" integer generated always as (0) not null, constraint "release_credits_has_artist" foreign key ("id", "first_artist_position") references "artists_release_credits" ("credit_id", "position") deferrable initially deferred) strict"#,
    r#"create unique index "release_credits_release_id_role_index" on "release_credits" ("release_id", "role")"#,
    r#"create table "artists_release_credits" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "credit_id" integer not null references "release_credits" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "artists_release_credits_credit_id_artist_id_index" on "artists_release_credits" ("credit_id", "artist_id")"#,
    r#"create unique index "artists_release_credits_credit_id_position_index" on "artists_release_credits" ("credit_id", "position")"#,
    r#"create index "artists_release_credits_artist_id_index" on "artists_release_credits" ("artist_id")"#,
    r#"create table "track_credits" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "role" text not null check ("role" <> '' AND "role" = trim("role")) COLLATE NOCASE, "position" integer not null check (position >= 0), "first_artist_position" integer generated always as (0) not null, constraint "track_credits_has_artist" foreign key ("id", "first_artist_position") references "artists_track_credits" ("credit_id", "position") deferrable initially deferred) strict"#,
    r#"create unique index "track_credits_track_id_role_index" on "track_credits" ("track_id", "role")"#,
    r#"create table "artists_track_credits" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "credit_id" integer not null references "track_credits" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "artists_track_credits_credit_id_artist_id_index" on "artists_track_credits" ("credit_id", "artist_id")"#,
    r#"create unique index "artists_track_credits_credit_id_position_index" on "artists_track_credits" ("credit_id", "position")"#,
    r#"create index "artists_track_credits_artist_id_index" on "artists_track_credits" ("artist_id")"#,
];

const DOWN: &[&str] = &[
    r#"drop table "artists_release_credits""#,
    r#"drop table "release_credits""#,
    r#"drop table "artists_track_credits""#,
    r#"drop table "track_credits""#,
    r#"create table "credit_roles" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE) strict"#,
    r#"create unique index "credit_roles_name_index" on "credit_roles" ("name")"#,
    r#"create table "release_credits" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "artist_id" integer not null references "artists" ("id") on delete restrict, "role_id" integer not null references "credit_roles" ("id") on delete restrict, "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "release_credits_release_id_artist_id_role_id_index" on "release_credits" ("release_id", "artist_id", "role_id")"#,
    r#"create index "release_credits_artist_id_index" on "release_credits" ("artist_id")"#,
    r#"create index "release_credits_role_id_index" on "release_credits" ("role_id")"#,
    r#"create table "track_credits" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "artist_id" integer not null references "artists" ("id") on delete restrict, "role_id" integer not null references "credit_roles" ("id") on delete restrict, "position" integer not null check (position >= 0)) strict"#,
    r#"create unique index "track_credits_track_id_artist_id_role_id_index" on "track_credits" ("track_id", "artist_id", "role_id")"#,
    r#"create index "track_credits_artist_id_index" on "track_credits" ("artist_id")"#,
    r#"create index "track_credits_role_id_index" on "track_credits" ("role_id")"#,
    r#"INSERT INTO credit_roles (name)
    VALUES ('composer'), ('conductor'), ('producer'), ('remixer'), ('writer')"#,
];
