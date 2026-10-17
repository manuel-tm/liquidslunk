//! Tags on tracks: a value table per type, built like the releases' and
//! using the same tag definitions. Switching a tag to single-value now
//! checks tracks too.

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
    r#"create table "track_text_tags" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "tag_id" integer not null references "text_tags" ("id") on delete restrict, "value" text not null check (value <> '' AND value = trim(value)) COLLATE NOCASE) strict"#,
    r#"create unique index "track_text_tags_track_id_tag_id_value_index" on "track_text_tags" ("track_id", "tag_id", "value")"#,
    r#"create index "track_text_tags_tag_id_value_index" on "track_text_tags" ("tag_id", "value")"#,
    r#"CREATE TRIGGER track_text_tags_single_value_insert
        BEFORE INSERT ON track_text_tags
        WHEN (SELECT multiple FROM text_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_text_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END"#,
    r#"CREATE TRIGGER track_text_tags_single_value_update
        BEFORE UPDATE OF track_id, tag_id ON track_text_tags
        WHEN (SELECT multiple FROM text_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_text_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END"#,
    r#"DROP TRIGGER text_tags_multiple_off"#,
    r#"CREATE TRIGGER text_tags_multiple_off
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
    END"#,
    r#"create table "track_real_number_tags" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "tag_id" integer not null references "real_number_tags" ("id") on delete restrict, "value" real not null) strict"#,
    r#"create unique index "track_real_number_tags_track_id_tag_id_value_index" on "track_real_number_tags" ("track_id", "tag_id", "value")"#,
    r#"create index "track_real_number_tags_tag_id_value_index" on "track_real_number_tags" ("tag_id", "value")"#,
    r#"CREATE TRIGGER track_real_number_tags_single_value_insert
        BEFORE INSERT ON track_real_number_tags
        WHEN (SELECT multiple FROM real_number_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_real_number_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END"#,
    r#"CREATE TRIGGER track_real_number_tags_single_value_update
        BEFORE UPDATE OF track_id, tag_id ON track_real_number_tags
        WHEN (SELECT multiple FROM real_number_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_real_number_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END"#,
    r#"CREATE TRIGGER track_real_number_tags_value_is_finite_insert
            BEFORE INSERT ON track_real_number_tags
            WHEN abs(NEW.value) = 9e999
            BEGIN
              SELECT RAISE(ABORT, 'a number tag''s value must be finite');
            END"#,
    r#"CREATE TRIGGER track_real_number_tags_value_is_finite_update
            BEFORE UPDATE OF value ON track_real_number_tags
            WHEN abs(NEW.value) = 9e999
            BEGIN
              SELECT RAISE(ABORT, 'a number tag''s value must be finite');
            END"#,
    r#"DROP TRIGGER real_number_tags_multiple_off"#,
    r#"CREATE TRIGGER real_number_tags_multiple_off
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
    END"#,
    r#"create table "track_date_tags" ("id" integer not null primary key, "track_id" integer not null references "tracks" ("id") on delete cascade, "tag_id" integer not null references "date_tags" ("id") on delete restrict, "value" integer not null) strict"#,
    r#"create unique index "track_date_tags_track_id_tag_id_value_index" on "track_date_tags" ("track_id", "tag_id", "value")"#,
    r#"create index "track_date_tags_tag_id_value_index" on "track_date_tags" ("tag_id", "value")"#,
    r#"CREATE TRIGGER track_date_tags_single_value_insert
        BEFORE INSERT ON track_date_tags
        WHEN (SELECT multiple FROM date_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_date_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END"#,
    r#"CREATE TRIGGER track_date_tags_single_value_update
        BEFORE UPDATE OF track_id, tag_id ON track_date_tags
        WHEN (SELECT multiple FROM date_tags WHERE id = NEW.tag_id) = 0
          AND EXISTS (SELECT 1 FROM track_date_tags
                      WHERE track_id = NEW.track_id AND tag_id = NEW.tag_id
                        AND id <> NEW.id)
        BEGIN
          SELECT RAISE(ABORT, 'single-value tag already set on this track');
        END"#,
    r#"CREATE TRIGGER track_date_tags_value_is_day_insert
            BEFORE INSERT ON track_date_tags
            WHEN NEW.value % 86400 <> 0
            BEGIN
              SELECT RAISE(ABORT, 'a date tag''s value must be a day, at midnight UTC');
            END"#,
    r#"CREATE TRIGGER track_date_tags_value_is_day_update
            BEFORE UPDATE OF value ON track_date_tags
            WHEN NEW.value % 86400 <> 0
            BEGIN
              SELECT RAISE(ABORT, 'a date tag''s value must be a day, at midnight UTC');
            END"#,
    r#"DROP TRIGGER date_tags_multiple_off"#,
    r#"CREATE TRIGGER date_tags_multiple_off
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
    END"#,
];

const DOWN: &[&str] = &[
    r#"drop table "track_text_tags""#,
    r#"DROP TRIGGER text_tags_multiple_off"#,
    r#"CREATE TRIGGER text_tags_multiple_off
    BEFORE UPDATE OF multiple ON text_tags
    WHEN OLD.multiple = 1 AND NEW.multiple = 0
      AND (EXISTS (SELECT 1 FROM release_text_tags WHERE tag_id = NEW.id
                    GROUP BY release_id
                    HAVING count(*) > 1))
    BEGIN
      SELECT RAISE(ABORT, 'some releases have several values for this tag');
    END"#,
    r#"drop table "track_real_number_tags""#,
    r#"DROP TRIGGER real_number_tags_multiple_off"#,
    r#"CREATE TRIGGER real_number_tags_multiple_off
    BEFORE UPDATE OF multiple ON real_number_tags
    WHEN OLD.multiple = 1 AND NEW.multiple = 0
      AND (EXISTS (SELECT 1 FROM release_real_number_tags WHERE tag_id = NEW.id
                    GROUP BY release_id
                    HAVING count(*) > 1))
    BEGIN
      SELECT RAISE(ABORT, 'some releases have several values for this tag');
    END"#,
    r#"drop table "track_date_tags""#,
    r#"DROP TRIGGER date_tags_multiple_off"#,
    r#"CREATE TRIGGER date_tags_multiple_off
    BEFORE UPDATE OF multiple ON date_tags
    WHEN OLD.multiple = 1 AND NEW.multiple = 0
      AND (EXISTS (SELECT 1 FROM release_date_tags WHERE tag_id = NEW.id
                    GROUP BY release_id
                    HAVING count(*) > 1))
    BEGIN
      SELECT RAISE(ABORT, 'some releases have several values for this tag');
    END"#,
];
