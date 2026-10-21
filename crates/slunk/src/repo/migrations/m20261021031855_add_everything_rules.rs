//! Everything rules (database.md, "External libraries"): a rule can name the
//! whole library, now and later, which left-out releases and tracks still
//! win over. It's an `everything` column on `external_library_rules`, which
//! the table's "names one thing" CHECK counts, so the table is rebuilt.
//!
//! - An everything rule names nothing else, and can't be left out.
//! - An external library has one at most.
//! - Rolling back deletes the everything rules, which the old table can't
//!   hold.

use rusqlite::Transaction;

use super::run;
use crate::Result;
use crate::repo::rebuild_table::{Rebuild, rebuild_table};

pub fn up(tx: &Transaction) -> Result<()> {
    rebuild_table(
        tx,
        "external_library_rules",
        Rebuild {
            define: UP_DEFINE,
            copy: &[],
        },
    )?;
    run(tx, &[CREATE_INDEX])
}

pub fn down(tx: &Transaction) -> Result<()> {
    run(
        tx,
        &[
            DROP_INDEX,
            r#"delete from "external_library_rules" where "everything" = 1"#,
        ],
    )?;
    rebuild_table(
        tx,
        "external_library_rules",
        Rebuild {
            define: DOWN_DEFINE,
            copy: &[],
        },
    )
}

const UP_DEFINE: &str = r#"("id" integer not null primary key, "external_library_id" integer not null references "external_libraries" ("id") on delete cascade, "exclude" integer default 0 not null check ("exclude" IN (0, 1)), "release_id" integer references "releases" ("id") on delete cascade, "artist_id" integer references "artists" ("id") on delete cascade, "track_id" integer references "tracks" ("id") on delete cascade, "text_tag_id" integer references "text_tags" ("id") on delete cascade, "real_number_tag_id" integer references "real_number_tags" ("id") on delete cascade, "date_tag_id" integer references "date_tags" ("id") on delete cascade, "everything" integer default 0 not null check ("everything" IN (0, 1)), "tag_equals" text check ("tag_equals" <> '' AND "tag_equals" = trim("tag_equals")) COLLATE NOCASE, "tag_min" text check ("tag_min" <> '' AND "tag_min" = trim("tag_min")), "tag_max" text check ("tag_max" <> '' AND "tag_max" = trim("tag_max")), constraint "external_library_rules_name_one_thing" check (("release_id" IS NOT NULL) + ("artist_id" IS NOT NULL) + ("track_id" IS NOT NULL) + ("text_tag_id" IS NOT NULL) + ("real_number_tag_id" IS NOT NULL) + ("date_tag_id" IS NOT NULL) + ("everything" = 1) = 1), constraint "external_library_rules_tag_value_has_its_filter" check (CASE WHEN "text_tag_id" IS NOT NULL THEN "tag_equals" IS NOT NULL AND "tag_min" IS NULL AND "tag_max" IS NULL WHEN "real_number_tag_id" IS NOT NULL OR "date_tag_id" IS NOT NULL THEN ("tag_equals" IS NOT NULL) <> ("tag_min" IS NOT NULL OR "tag_max" IS NOT NULL) ELSE "tag_equals" IS NULL AND "tag_min" IS NULL AND "tag_max" IS NULL END), constraint "external_library_rules_exclude_a_release_or_track" check ("exclude" = 0 OR "release_id" IS NOT NULL OR "track_id" IS NOT NULL)) strict"#;

const CREATE_INDEX: &str = r#"create unique index "external_library_rules_external_library_id_everything_index" on "external_library_rules" ("external_library_id") where "everything" = 1"#;

const DROP_INDEX: &str = r#"drop index "external_library_rules_external_library_id_everything_index""#;

// add_external_libraries' definition, as it was
const DOWN_DEFINE: &str = r#"("id" integer not null primary key, "external_library_id" integer not null references "external_libraries" ("id") on delete cascade, "exclude" integer default 0 not null check ("exclude" IN (0, 1)), "release_id" integer references "releases" ("id") on delete cascade, "artist_id" integer references "artists" ("id") on delete cascade, "track_id" integer references "tracks" ("id") on delete cascade, "text_tag_id" integer references "text_tags" ("id") on delete cascade, "real_number_tag_id" integer references "real_number_tags" ("id") on delete cascade, "date_tag_id" integer references "date_tags" ("id") on delete cascade, "tag_equals" text check ("tag_equals" <> '' AND "tag_equals" = trim("tag_equals")) COLLATE NOCASE, "tag_min" text check ("tag_min" <> '' AND "tag_min" = trim("tag_min")), "tag_max" text check ("tag_max" <> '' AND "tag_max" = trim("tag_max")), constraint "external_library_rules_name_one_thing" check (("release_id" IS NOT NULL) + ("artist_id" IS NOT NULL) + ("track_id" IS NOT NULL) + ("text_tag_id" IS NOT NULL) + ("real_number_tag_id" IS NOT NULL) + ("date_tag_id" IS NOT NULL) = 1), constraint "external_library_rules_tag_value_has_its_filter" check (CASE WHEN "text_tag_id" IS NOT NULL THEN "tag_equals" IS NOT NULL AND "tag_min" IS NULL AND "tag_max" IS NULL WHEN "real_number_tag_id" IS NOT NULL OR "date_tag_id" IS NOT NULL THEN ("tag_equals" IS NOT NULL) <> ("tag_min" IS NOT NULL OR "tag_max" IS NOT NULL) ELSE "tag_equals" IS NULL AND "tag_min" IS NULL AND "tag_max" IS NULL END), constraint "external_library_rules_exclude_a_release_or_track" check ("exclude" = 0 OR "release_id" IS NOT NULL OR "track_id" IS NOT NULL)) strict"#;
