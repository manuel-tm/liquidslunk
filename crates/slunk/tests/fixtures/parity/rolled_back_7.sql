PRAGMA foreign_keys=OFF;
BEGIN TRANSACTION;
CREATE TABLE IF NOT EXISTS "migrations" ("id" integer not null primary key, "name" text not null unique, "applied_at" integer default (unixepoch()) not null) strict;
INSERT INTO migrations VALUES(1,'20261011141955_create_baseline',1792234848);
INSERT INTO migrations VALUES(2,'20261013060918_add_tag_value_rules',1792234848);
INSERT INTO migrations VALUES(3,'20261014052413_add_track_tags',1792234848);
INSERT INTO migrations VALUES(4,'20261015192802_free_text_credit_roles',1792234848);
INSERT INTO migrations VALUES(5,'20261015195853_add_disc_titles',1792234848);
INSERT INTO migrations VALUES(6,'20261016144030_add_alternate_names',1792234848);
CREATE TABLE IF NOT EXISTS "files" ("id" integer not null primary key autoincrement, "path" text not null check (path <> '' AND substr(path, 1, 1) <> '/'), "type" text not null check (type IN ('music', 'image', 'other')), "size_bytes" integer not null check (size_bytes >= 0), "mtime" integer not null, "sha256" text not null check (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'), "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null) strict;
INSERT INTO files VALUES(1,'Scenery/01 It Could Happen to You.flac','music',31457280,1700000000,'0000000000000000000000000000000000000000000000000000000000000001',1700000100,1700000100);
INSERT INTO files VALUES(2,'Scenery/02 Early Summer.flac','music',29360128,1700000001,'0000000000000000000000000000000000000000000000000000000000000002',1700000100,1700000100);
INSERT INTO files VALUES(3,'Scenery/cover.jpg','image',524288,1700000002,'0000000000000000000000000000000000000000000000000000000000000003',1700000100,1700000100);
INSERT INTO files VALUES(4,'Virmalised/01 Intro.flac','music',10485760,1700000003,'0000000000000000000000000000000000000000000000000000000000000004',1700000100,1700000100);
INSERT INTO files VALUES(5,'Virmalised/CD2/01 Virmalised.flac','music',20971520,1700000004,'0000000000000000000000000000000000000000000000000000000000000005',1700000100,1700000100);
INSERT INTO files VALUES(6,'Artists/alika.jpg','image',262144,1700000005,'0000000000000000000000000000000000000000000000000000000000000006',1700000100,1700000100);
INSERT INTO files VALUES(7,'../Other/notes.txt','other',1024,1700000006,'0000000000000000000000000000000000000000000000000000000000000007',1700000100,1700000100);
CREATE TABLE IF NOT EXISTS "artists" ("id" integer not null primary key autoincrement, "name" text not null check ("name" <> '' AND "name" = trim("name")), "picture" integer references "files" ("id") on delete restrict, "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null, "name_language" text check (("name_language" GLOB '[a-z][a-z]' OR "name_language" GLOB '[a-z][a-z][a-z]'
    OR "name_language" GLOB '[a-z][a-z]-?*' OR "name_language" GLOB '[a-z][a-z][a-z]-?*')
    AND "name_language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "name_language" NOT GLOB '*--*' AND "name_language" NOT GLOB '*-'
    AND "name_language" <> 'und' AND "name_language" NOT GLOB 'und-*')) strict;
INSERT INTO artists VALUES(1,'Ryo Fukui',NULL,1700000200,1700000200,'ja-Latn');
INSERT INTO artists VALUES(2,'Alika',6,1700000200,1700000300,NULL);
INSERT INTO artists VALUES(3,'Sam Jones',NULL,1700000200,1700000200,NULL);
CREATE TABLE IF NOT EXISTS "labels" ("id" integer not null primary key autoincrement, "name" text not null check ("name" <> '' AND "name" = trim("name")), "picture" integer references "files" ("id") on delete restrict, "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null, "name_language" text check (("name_language" GLOB '[a-z][a-z]' OR "name_language" GLOB '[a-z][a-z][a-z]'
    OR "name_language" GLOB '[a-z][a-z]-?*' OR "name_language" GLOB '[a-z][a-z][a-z]-?*')
    AND "name_language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "name_language" NOT GLOB '*--*' AND "name_language" NOT GLOB '*-'
    AND "name_language" <> 'und' AND "name_language" NOT GLOB 'und-*')) strict;
INSERT INTO labels VALUES(1,'Trio Records',NULL,1700000200,1700000200,'en');
INSERT INTO labels VALUES(2,'Self-released',NULL,1700000200,1700000200,NULL);
CREATE TABLE IF NOT EXISTS "releases" ("id" integer not null primary key autoincrement, "title" text not null check ("title" <> '' AND "title" = trim("title")), "release_date" integer, "release_date_precision" text check (release_date_precision IN ('year', 'month', 'day')), "catalog_number" text check ("catalog_number" <> '' AND "catalog_number" = trim("catalog_number")) COLLATE NOCASE, "cover_picture" integer references "files" ("id") on delete restrict, "various_artists" integer default 0 not null check (various_artists IN (0, 1)), "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null, constraint "releases_release_date_precision_set" check ((release_date IS NULL) = (release_date_precision IS NULL)), constraint "releases_release_date_matches_precision" check (release_date IS NULL OR (
        release_date % 86400 = 0
        AND (release_date_precision <> 'month'
             OR strftime('%d', release_date, 'unixepoch') = '01')
        AND (release_date_precision <> 'year'
             OR strftime('%m-%d', release_date, 'unixepoch') = '01-01')
      ))) strict;
INSERT INTO releases VALUES(1,'Scenery',189302400,'year','PAP-9019',3,0,1700000400,1700000400);
INSERT INTO releases VALUES(2,'Virmalised',1525132800,'month',NULL,NULL,0,1700000400,1700000400);
INSERT INTO releases VALUES(3,'Jazz Sampler',NULL,NULL,NULL,NULL,1,1700000400,1700000400);
CREATE TABLE IF NOT EXISTS "tracks" ("id" integer not null primary key autoincrement, "title" text not null check ("title" <> '' AND "title" = trim("title")), "duration_ms" integer check (duration_ms >= 0), "track_number" integer not null check (track_number > 0), "cd_number" integer default 1 not null check (cd_number > 0), "release_id" integer not null references "releases" ("id") on delete restrict, "file_id" integer not null references "files" ("id") on delete restrict, "inserted_at" integer default (unixepoch()) not null, "updated_at" integer default (unixepoch()) not null) strict;
INSERT INTO tracks VALUES(1,'It Could Happen to You',452000,1,1,1,1,1700000500,1700000500);
INSERT INTO tracks VALUES(2,'Early Summer',431000,2,1,1,2,1700000500,1700000500);
INSERT INTO tracks VALUES(3,'Intro',120000,1,1,2,4,1700000500,1700000500);
INSERT INTO tracks VALUES(4,'Virmalised',NULL,1,2,2,5,1700000500,1700000500);
CREATE TABLE IF NOT EXISTS "labels_releases" ("id" integer not null primary key, "label_id" integer not null references "labels" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict;
INSERT INTO labels_releases VALUES(1,1,1,0);
INSERT INTO labels_releases VALUES(2,2,2,0);
CREATE TABLE IF NOT EXISTS "release_formats" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE) strict;
INSERT INTO release_formats VALUES(1,'Album');
INSERT INTO release_formats VALUES(2,'Compilation');
INSERT INTO release_formats VALUES(3,'EP');
INSERT INTO release_formats VALUES(4,'Live');
INSERT INTO release_formats VALUES(5,'LP');
INSERT INTO release_formats VALUES(6,'Mixtape');
INSERT INTO release_formats VALUES(7,'Single');
INSERT INTO release_formats VALUES(8,'Soundtrack');
CREATE TABLE IF NOT EXISTS "release_formats_releases" ("id" integer not null primary key, "format_id" integer not null references "release_formats" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade) strict;
INSERT INTO release_formats_releases VALUES(1,1,1);
INSERT INTO release_formats_releases VALUES(2,3,2);
CREATE TABLE IF NOT EXISTS "release_media" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE) strict;
INSERT INTO release_media VALUES(1,'Cassette');
INSERT INTO release_media VALUES(2,'CD');
INSERT INTO release_media VALUES(3,'Digital');
INSERT INTO release_media VALUES(4,'Vinyl');
CREATE TABLE IF NOT EXISTS "release_media_releases" ("id" integer not null primary key, "medium_id" integer not null references "release_media" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade) strict;
INSERT INTO release_media_releases VALUES(1,2,1);
INSERT INTO release_media_releases VALUES(2,3,2);
CREATE TABLE IF NOT EXISTS "files_releases" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "file_id" integer not null references "files" ("id") on delete restrict, "note" text check ("note" <> '' AND "note" = trim("note"))) strict;
INSERT INTO files_releases VALUES(1,1,7,'Liner notes');
CREATE TABLE IF NOT EXISTS "artists_files" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete cascade, "file_id" integer not null references "files" ("id") on delete restrict, "note" text check ("note" <> '' AND "note" = trim("note"))) strict;
INSERT INTO artists_files VALUES(1,2,6,NULL);
CREATE TABLE IF NOT EXISTS "files_tracks" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "file_id" integer not null references "files" ("id") on delete restrict, "note" text check ("note" <> '' AND "note" = trim("note"))) strict;
INSERT INTO files_tracks VALUES(1,1,3,'Back cover');
CREATE TABLE IF NOT EXISTS "text_tags" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE, "multiple" integer default 0 not null check (multiple IN (0, 1))) strict;
INSERT INTO text_tags VALUES(1,'Genre',1);
CREATE TABLE IF NOT EXISTS "release_text_tags" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "tag_id" integer not null references "text_tags" ("id") on delete restrict, "value" text not null check ("value" <> '' AND "value" = trim("value")) COLLATE NOCASE) strict;
INSERT INTO release_text_tags VALUES(1,1,1,'Jazz');
INSERT INTO release_text_tags VALUES(2,1,1,'Bebop');
CREATE TABLE IF NOT EXISTS "real_number_tags" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE, "multiple" integer default 0 not null check (multiple IN (0, 1))) strict;
INSERT INTO real_number_tags VALUES(1,'Score',0);
CREATE TABLE IF NOT EXISTS "release_real_number_tags" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "tag_id" integer not null references "real_number_tags" ("id") on delete restrict, "value" real not null) strict;
INSERT INTO release_real_number_tags VALUES(1,1,1,9.5);
INSERT INTO release_real_number_tags VALUES(2,2,1,-3.25);
CREATE TABLE IF NOT EXISTS "date_tags" ("id" integer not null primary key, "name" text not null check ("name" <> '' AND "name" = trim("name")) COLLATE NOCASE, "multiple" integer default 0 not null check (multiple IN (0, 1))) strict;
INSERT INTO date_tags VALUES(1,'Purchased',0);
CREATE TABLE IF NOT EXISTS "release_date_tags" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "tag_id" integer not null references "date_tags" ("id") on delete restrict, "value" integer not null) strict;
INSERT INTO release_date_tags VALUES(1,2,1,1599955200);
INSERT INTO release_date_tags VALUES(2,1,1,1600041600);
CREATE TABLE IF NOT EXISTS "track_text_tags" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "tag_id" integer not null references "text_tags" ("id") on delete restrict, "value" text not null check (value <> '' AND value = trim(value)) COLLATE NOCASE) strict;
INSERT INTO track_text_tags VALUES(1,1,1,'Jazz');
INSERT INTO track_text_tags VALUES(2,1,1,'Ballad');
CREATE TABLE IF NOT EXISTS "track_real_number_tags" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "tag_id" integer not null references "real_number_tags" ("id") on delete restrict, "value" real not null) strict;
INSERT INTO track_real_number_tags VALUES(1,3,1,7.0);
CREATE TABLE IF NOT EXISTS "track_date_tags" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "tag_id" integer not null references "date_tags" ("id") on delete restrict, "value" integer not null) strict;
INSERT INTO track_date_tags VALUES(1,2,1,1600128000);
CREATE TABLE IF NOT EXISTS "release_credits" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "role" text not null check ("role" <> '' AND "role" = trim("role")) COLLATE NOCASE, "position" integer not null check (position >= 0), "first_artist_position" integer generated always as (0) not null, constraint "release_credits_has_artist" foreign key ("id", "first_artist_position") references "artists_release_credits" ("credit_id", "position") deferrable initially deferred) strict;
INSERT INTO release_credits VALUES(1,1,'Producer',0);
INSERT INTO release_credits VALUES(2,2,'Mixed By',0);
CREATE TABLE IF NOT EXISTS "track_credits" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "role" text not null check ("role" <> '' AND "role" = trim("role")) COLLATE NOCASE, "position" integer not null check (position >= 0), "first_artist_position" integer generated always as (0) not null, constraint "track_credits_has_artist" foreign key ("id", "first_artist_position") references "artists_track_credits" ("credit_id", "position") deferrable initially deferred) strict;
INSERT INTO track_credits VALUES(1,1,'Composed By',0);
CREATE TABLE IF NOT EXISTS "release_disc_titles" ("id" integer not null primary key, "release_id" integer not null references "releases" ("id") on delete cascade, "cd_number" integer not null check (cd_number > 0), "title" text not null check ("title" <> '' AND "title" = trim("title"))) strict;
INSERT INTO release_disc_titles VALUES(1,2,2,'Live at Tallinn');
CREATE TABLE IF NOT EXISTS "artist_alternate_names" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete cascade, "name" text not null check ("name" <> '' AND "name" = trim("name")), "language" text check (("language" GLOB '[a-z][a-z]' OR "language" GLOB '[a-z][a-z][a-z]'
    OR "language" GLOB '[a-z][a-z]-?*' OR "language" GLOB '[a-z][a-z][a-z]-?*')
    AND "language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "language" NOT GLOB '*--*' AND "language" NOT GLOB '*-'
    AND "language" <> 'und' AND "language" NOT GLOB 'und-*'), "real_name" integer default 0 not null check (real_name IN (0, 1)), "note" text check ("note" <> '' AND "note" = trim("note")), "position" integer not null check (position >= 0)) strict;
INSERT INTO artist_alternate_names VALUES(1,2,'ALIKA',NULL,0,NULL,0);
INSERT INTO artist_alternate_names VALUES(2,2,'Alika Milova','et',1,'Birth name',1);
INSERT INTO artist_alternate_names VALUES(3,1,'福居良','ja',0,NULL,0);
CREATE TABLE IF NOT EXISTS "label_alternate_names" ("id" integer not null primary key, "label_id" integer not null references "labels" ("id") on delete cascade, "name" text not null check ("name" <> '' AND "name" = trim("name")), "language" text check (("language" GLOB '[a-z][a-z]' OR "language" GLOB '[a-z][a-z][a-z]'
    OR "language" GLOB '[a-z][a-z]-?*' OR "language" GLOB '[a-z][a-z][a-z]-?*')
    AND "language" NOT GLOB '*[^A-Za-z0-9-]*'
    AND "language" NOT GLOB '*--*' AND "language" NOT GLOB '*-'
    AND "language" <> 'und' AND "language" NOT GLOB 'und-*'), "note" text check ("note" <> '' AND "note" = trim("note")), "position" integer not null check (position >= 0)) strict;
INSERT INTO label_alternate_names VALUES(1,1,'トリオ・レコード','ja',NULL,0);
CREATE TABLE IF NOT EXISTS "artists_releases" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "release_id" integer not null references "releases" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict;
INSERT INTO artists_releases VALUES(1,1,1,0);
INSERT INTO artists_releases VALUES(2,2,2,0);
CREATE TABLE IF NOT EXISTS "artists_tracks" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "track_id" integer not null references "tracks" ("id") on delete cascade, "featured" integer default 0 not null check (featured IN (0, 1)), "position" integer not null check (position >= 0)) strict;
INSERT INTO artists_tracks VALUES(1,3,1,1,0);
INSERT INTO artists_tracks VALUES(2,2,3,0,0);
CREATE TABLE IF NOT EXISTS "artists_release_credits" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "credit_id" integer not null references "release_credits" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict;
INSERT INTO artists_release_credits VALUES(1,3,1,0);
INSERT INTO artists_release_credits VALUES(2,1,1,1);
INSERT INTO artists_release_credits VALUES(3,2,2,0);
CREATE TABLE IF NOT EXISTS "artists_track_credits" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id") on delete restrict, "credit_id" integer not null references "track_credits" ("id") on delete cascade, "position" integer not null check (position >= 0)) strict;
INSERT INTO artists_track_credits VALUES(1,1,1,0);
PRAGMA writable_schema=ON;
CREATE TABLE IF NOT EXISTS sqlite_sequence(name,seq);
DELETE FROM sqlite_sequence;
INSERT INTO sqlite_sequence VALUES('files',8);
INSERT INTO sqlite_sequence VALUES('artists',3);
INSERT INTO sqlite_sequence VALUES('labels',2);
INSERT INTO sqlite_sequence VALUES('releases',3);
INSERT INTO sqlite_sequence VALUES('tracks',4);
CREATE TRIGGER release_text_tags_single_value_insert
        BEFORE INSERT ON release_text_tags
        WHEN (SELECT multiple FROM text_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_text_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END;
CREATE TRIGGER release_text_tags_single_value_update
        BEFORE UPDATE OF release_id, tag_id ON release_text_tags
        WHEN (SELECT multiple FROM text_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_text_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END;
CREATE TRIGGER release_real_number_tags_single_value_insert
        BEFORE INSERT ON release_real_number_tags
        WHEN (SELECT multiple FROM real_number_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_real_number_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END;
CREATE TRIGGER release_real_number_tags_single_value_update
        BEFORE UPDATE OF release_id, tag_id ON release_real_number_tags
        WHEN (SELECT multiple FROM real_number_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_real_number_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END;
CREATE TRIGGER release_date_tags_single_value_insert
        BEFORE INSERT ON release_date_tags
        WHEN (SELECT multiple FROM date_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_date_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END;
CREATE TRIGGER release_date_tags_single_value_update
        BEFORE UPDATE OF release_id, tag_id ON release_date_tags
        WHEN (SELECT multiple FROM date_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM release_date_tags
                      WHERE release_id = NEW.release_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this release');
        END;
CREATE TRIGGER tracks_file_is_music_insert
    BEFORE INSERT ON tracks
    WHEN (SELECT type FROM files WHERE id = NEW.file_id) <> 'music'
    BEGIN
      SELECT RAISE(ABORT, 'a track''s file must be music');
    END;
CREATE TRIGGER tracks_file_is_music_update
    BEFORE UPDATE OF file_id ON tracks
    WHEN (SELECT type FROM files WHERE id = NEW.file_id) <> 'music'
    BEGIN
      SELECT RAISE(ABORT, 'a track''s file must be music');
    END;
CREATE TRIGGER files_type_stays_music
    BEFORE UPDATE OF type ON files
    WHEN NEW.type <> 'music'
      AND EXISTS (SELECT 1 FROM tracks WHERE file_id = NEW.id)
    BEGIN
      SELECT RAISE(ABORT, 'a track''s file must stay music');
    END;
CREATE TRIGGER releases_cover_picture_is_image_insert
          BEFORE INSERT ON releases
          WHEN (SELECT type FROM files WHERE id = NEW.cover_picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'a release cover must be an image');
          END;
CREATE TRIGGER releases_cover_picture_is_image_update
          BEFORE UPDATE OF cover_picture ON releases
          WHEN (SELECT type FROM files WHERE id = NEW.cover_picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'a release cover must be an image');
          END;
CREATE TRIGGER artists_picture_is_image_insert
          BEFORE INSERT ON artists
          WHEN (SELECT type FROM files WHERE id = NEW.picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'an artist picture must be an image');
          END;
CREATE TRIGGER artists_picture_is_image_update
          BEFORE UPDATE OF picture ON artists
          WHEN (SELECT type FROM files WHERE id = NEW.picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'an artist picture must be an image');
          END;
CREATE TRIGGER labels_picture_is_image_insert
          BEFORE INSERT ON labels
          WHEN (SELECT type FROM files WHERE id = NEW.picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'a label picture must be an image');
          END;
CREATE TRIGGER labels_picture_is_image_update
          BEFORE UPDATE OF picture ON labels
          WHEN (SELECT type FROM files WHERE id = NEW.picture) <> 'image'
          BEGIN
            SELECT RAISE(ABORT, 'a label picture must be an image');
          END;
CREATE TRIGGER files_type_stays_image
    BEFORE UPDATE OF type ON files
    WHEN NEW.type <> 'image'
      AND (EXISTS (SELECT 1 FROM releases WHERE cover_picture = NEW.id)
           OR EXISTS (SELECT 1 FROM artists WHERE picture = NEW.id)
           OR EXISTS (SELECT 1 FROM labels WHERE picture = NEW.id))
    BEGIN
      SELECT RAISE(ABORT, 'a cover or picture must stay an image');
    END;
CREATE TRIGGER release_date_tags_value_is_day_insert
          BEFORE INSERT ON release_date_tags
          WHEN NEW.value % 86400 <> 0
          BEGIN
            SELECT RAISE(ABORT, 'a date tag''s value must be a day, at midnight UTC');
          END;
CREATE TRIGGER release_date_tags_value_is_day_update
          BEFORE UPDATE OF value ON release_date_tags
          WHEN NEW.value % 86400 <> 0
          BEGIN
            SELECT RAISE(ABORT, 'a date tag''s value must be a day, at midnight UTC');
          END;
CREATE TRIGGER release_real_number_tags_value_is_finite_insert
          BEFORE INSERT ON release_real_number_tags
          WHEN abs(NEW.value) = 9e999
          BEGIN
            SELECT RAISE(ABORT, 'a number tag''s value must be finite');
          END;
CREATE TRIGGER release_real_number_tags_value_is_finite_update
          BEFORE UPDATE OF value ON release_real_number_tags
          WHEN abs(NEW.value) = 9e999
          BEGIN
            SELECT RAISE(ABORT, 'a number tag''s value must be finite');
          END;
CREATE TRIGGER track_text_tags_single_value_insert
        BEFORE INSERT ON track_text_tags
        WHEN (SELECT multiple FROM text_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_text_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END;
CREATE TRIGGER track_text_tags_single_value_update
        BEFORE UPDATE OF track_id, tag_id ON track_text_tags
        WHEN (SELECT multiple FROM text_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_text_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END;
CREATE TRIGGER text_tags_multiple_off
    BEFORE UPDATE OF multiple ON text_tags
    WHEN OLD.multiple = 1 AND NEW.multiple = 0
      AND (EXISTS (SELECT 1 FROM release_text_tags WHERE tag_id = NEW.id
                    GROUP BY release_id
                    HAVING count(*) > 1)
          OR EXISTS (SELECT 1 FROM track_text_tags WHERE tag_id = NEW.id
                    GROUP BY track_id
                    HAVING count(*) > 1))
    BEGIN
      SELECT RAISE(ABORT, 'some releases or tracks have several values for this tag');
    END;
CREATE TRIGGER track_real_number_tags_single_value_insert
        BEFORE INSERT ON track_real_number_tags
        WHEN (SELECT multiple FROM real_number_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_real_number_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END;
CREATE TRIGGER track_real_number_tags_single_value_update
        BEFORE UPDATE OF track_id, tag_id ON track_real_number_tags
        WHEN (SELECT multiple FROM real_number_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_real_number_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END;
CREATE TRIGGER track_real_number_tags_value_is_finite_insert
            BEFORE INSERT ON track_real_number_tags
            WHEN abs(NEW.value) = 9e999
            BEGIN
              SELECT RAISE(ABORT, 'a number tag''s value must be finite');
            END;
CREATE TRIGGER track_real_number_tags_value_is_finite_update
            BEFORE UPDATE OF value ON track_real_number_tags
            WHEN abs(NEW.value) = 9e999
            BEGIN
              SELECT RAISE(ABORT, 'a number tag''s value must be finite');
            END;
CREATE TRIGGER real_number_tags_multiple_off
    BEFORE UPDATE OF multiple ON real_number_tags
    WHEN OLD.multiple = 1 AND NEW.multiple = 0
      AND (EXISTS (SELECT 1 FROM release_real_number_tags WHERE tag_id = NEW.id
                    GROUP BY release_id
                    HAVING count(*) > 1)
          OR EXISTS (SELECT 1 FROM track_real_number_tags WHERE tag_id = NEW.id
                    GROUP BY track_id
                    HAVING count(*) > 1))
    BEGIN
      SELECT RAISE(ABORT, 'some releases or tracks have several values for this tag');
    END;
CREATE TRIGGER track_date_tags_single_value_insert
        BEFORE INSERT ON track_date_tags
        WHEN (SELECT multiple FROM date_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_date_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END;
CREATE TRIGGER track_date_tags_single_value_update
        BEFORE UPDATE OF track_id, tag_id ON track_date_tags
        WHEN (SELECT multiple FROM date_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_date_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END;
CREATE TRIGGER track_date_tags_value_is_day_insert
            BEFORE INSERT ON track_date_tags
            WHEN NEW.value % 86400 <> 0
            BEGIN
              SELECT RAISE(ABORT, 'a date tag''s value must be a day, at midnight UTC');
            END;
CREATE TRIGGER track_date_tags_value_is_day_update
            BEFORE UPDATE OF value ON track_date_tags
            WHEN NEW.value % 86400 <> 0
            BEGIN
              SELECT RAISE(ABORT, 'a date tag''s value must be a day, at midnight UTC');
            END;
CREATE TRIGGER date_tags_multiple_off
    BEFORE UPDATE OF multiple ON date_tags
    WHEN OLD.multiple = 1 AND NEW.multiple = 0
      AND (EXISTS (SELECT 1 FROM release_date_tags WHERE tag_id = NEW.id
                    GROUP BY release_id
                    HAVING count(*) > 1)
          OR EXISTS (SELECT 1 FROM track_date_tags WHERE tag_id = NEW.id
                    GROUP BY track_id
                    HAVING count(*) > 1))
    BEGIN
      SELECT RAISE(ABORT, 'some releases or tracks have several values for this tag');
    END;
CREATE TRIGGER release_disc_titles_disc_has_tracks_insert
        BEFORE INSERT ON release_disc_titles
        WHEN NOT EXISTS (
          SELECT 1 FROM tracks
          WHERE release_id = NEW.release_id AND cd_number = NEW.cd_number
        )
        BEGIN
          SELECT RAISE(ABORT, 'a disc title needs a track on its disc');
        END;
CREATE TRIGGER release_disc_titles_disc_has_tracks_update
        BEFORE UPDATE OF release_id, cd_number ON release_disc_titles
        WHEN NOT EXISTS (
          SELECT 1 FROM tracks
          WHERE release_id = NEW.release_id AND cd_number = NEW.cd_number
        )
        BEGIN
          SELECT RAISE(ABORT, 'a disc title needs a track on its disc');
        END;
CREATE TRIGGER tracks_empty_disc_loses_title_delete
        AFTER DELETE ON tracks
        
        BEGIN
          DELETE FROM release_disc_titles
          WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            AND NOT EXISTS (
              SELECT 1 FROM tracks
              WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            );
        END;
CREATE TRIGGER tracks_empty_disc_loses_title_update
        AFTER UPDATE OF release_id, cd_number ON tracks
        WHEN OLD.release_id <> NEW.release_id OR OLD.cd_number <> NEW.cd_number
        BEGIN
          DELETE FROM release_disc_titles
          WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            AND NOT EXISTS (
              SELECT 1 FROM tracks
              WHERE release_id = OLD.release_id AND cd_number = OLD.cd_number
            );
        END;
CREATE UNIQUE INDEX "files_path_index" on "files" ("path");
CREATE INDEX "files_sha256_index" on "files" ("sha256");
CREATE INDEX "artists_name_index" on "artists" ("name" COLLATE NOCASE);
CREATE INDEX "artists_picture_index" on "artists" ("picture");
CREATE INDEX "labels_name_index" on "labels" ("name" COLLATE NOCASE);
CREATE INDEX "labels_picture_index" on "labels" ("picture");
CREATE INDEX "releases_title_index" on "releases" ("title" COLLATE NOCASE);
CREATE INDEX "releases_release_date_index" on "releases" ("release_date");
CREATE INDEX "releases_catalog_number_index" on "releases" ("catalog_number");
CREATE INDEX "releases_cover_picture_index" on "releases" ("cover_picture");
CREATE UNIQUE INDEX "tracks_release_id_cd_number_track_number_index" on "tracks" ("release_id", "cd_number", "track_number");
CREATE UNIQUE INDEX "tracks_file_id_index" on "tracks" ("file_id");
CREATE UNIQUE INDEX "labels_releases_release_id_label_id_index" on "labels_releases" ("release_id", "label_id");
CREATE INDEX "labels_releases_label_id_index" on "labels_releases" ("label_id");
CREATE UNIQUE INDEX "release_formats_name_index" on "release_formats" ("name");
CREATE UNIQUE INDEX "release_formats_releases_release_id_format_id_index" on "release_formats_releases" ("release_id", "format_id");
CREATE INDEX "release_formats_releases_format_id_index" on "release_formats_releases" ("format_id");
CREATE UNIQUE INDEX "release_media_name_index" on "release_media" ("name");
CREATE UNIQUE INDEX "release_media_releases_release_id_medium_id_index" on "release_media_releases" ("release_id", "medium_id");
CREATE INDEX "release_media_releases_medium_id_index" on "release_media_releases" ("medium_id");
CREATE UNIQUE INDEX "files_releases_release_id_file_id_index" on "files_releases" ("release_id", "file_id");
CREATE INDEX "files_releases_file_id_index" on "files_releases" ("file_id");
CREATE UNIQUE INDEX "artists_files_artist_id_file_id_index" on "artists_files" ("artist_id", "file_id");
CREATE INDEX "artists_files_file_id_index" on "artists_files" ("file_id");
CREATE UNIQUE INDEX "files_tracks_track_id_file_id_index" on "files_tracks" ("track_id", "file_id");
CREATE INDEX "files_tracks_file_id_index" on "files_tracks" ("file_id");
CREATE UNIQUE INDEX "text_tags_name_index" on "text_tags" ("name");
CREATE UNIQUE INDEX "release_text_tags_release_id_tag_id_value_index" on "release_text_tags" ("release_id", "tag_id", "value");
CREATE INDEX "release_text_tags_tag_id_value_index" on "release_text_tags" ("tag_id", "value");
CREATE UNIQUE INDEX "real_number_tags_name_index" on "real_number_tags" ("name");
CREATE UNIQUE INDEX "release_real_number_tags_release_id_tag_id_value_index" on "release_real_number_tags" ("release_id", "tag_id", "value");
CREATE INDEX "release_real_number_tags_tag_id_value_index" on "release_real_number_tags" ("tag_id", "value");
CREATE UNIQUE INDEX "date_tags_name_index" on "date_tags" ("name");
CREATE UNIQUE INDEX "release_date_tags_release_id_tag_id_value_index" on "release_date_tags" ("release_id", "tag_id", "value");
CREATE INDEX "release_date_tags_tag_id_value_index" on "release_date_tags" ("tag_id", "value");
CREATE UNIQUE INDEX "track_text_tags_track_id_tag_id_value_index" on "track_text_tags" ("track_id", "tag_id", "value");
CREATE INDEX "track_text_tags_tag_id_value_index" on "track_text_tags" ("tag_id", "value");
CREATE UNIQUE INDEX "track_real_number_tags_track_id_tag_id_value_index" on "track_real_number_tags" ("track_id", "tag_id", "value");
CREATE INDEX "track_real_number_tags_tag_id_value_index" on "track_real_number_tags" ("tag_id", "value");
CREATE UNIQUE INDEX "track_date_tags_track_id_tag_id_value_index" on "track_date_tags" ("track_id", "tag_id", "value");
CREATE INDEX "track_date_tags_tag_id_value_index" on "track_date_tags" ("tag_id", "value");
CREATE UNIQUE INDEX "release_credits_release_id_role_index" on "release_credits" ("release_id", "role");
CREATE UNIQUE INDEX "track_credits_track_id_role_index" on "track_credits" ("track_id", "role");
CREATE UNIQUE INDEX "release_disc_titles_release_id_cd_number_index" on "release_disc_titles" ("release_id", "cd_number");
CREATE UNIQUE INDEX "artist_alternate_names_artist_id_name_index" on "artist_alternate_names" ("artist_id", "name");
CREATE UNIQUE INDEX "label_alternate_names_label_id_name_index" on "label_alternate_names" ("label_id", "name");
CREATE UNIQUE INDEX "artist_alternate_names_artist_id_id_index" on "artist_alternate_names" ("artist_id", "id");
CREATE INDEX "artist_alternate_names_name_index" on "artist_alternate_names" (name COLLATE NOCASE);
CREATE UNIQUE INDEX "artists_releases_release_id_artist_id_index" on "artists_releases" ("release_id", "artist_id");
CREATE INDEX "artists_releases_artist_id_index" on "artists_releases" ("artist_id");
CREATE UNIQUE INDEX "artists_tracks_track_id_artist_id_index" on "artists_tracks" ("track_id", "artist_id");
CREATE INDEX "artists_tracks_artist_id_index" on "artists_tracks" ("artist_id");
CREATE UNIQUE INDEX "artists_release_credits_credit_id_artist_id_index" on "artists_release_credits" ("credit_id", "artist_id");
CREATE UNIQUE INDEX "artists_release_credits_credit_id_position_index" on "artists_release_credits" ("credit_id", "position");
CREATE INDEX "artists_release_credits_artist_id_index" on "artists_release_credits" ("artist_id");
CREATE UNIQUE INDEX "artists_track_credits_credit_id_artist_id_index" on "artists_track_credits" ("credit_id", "artist_id");
CREATE UNIQUE INDEX "artists_track_credits_credit_id_position_index" on "artists_track_credits" ("credit_id", "position");
CREATE INDEX "artists_track_credits_artist_id_index" on "artists_track_credits" ("artist_id");
PRAGMA writable_schema=OFF;
COMMIT;
