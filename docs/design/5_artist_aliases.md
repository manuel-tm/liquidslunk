# Artist aliases

> **Status: in design.**

Some artists release under several names, each a project of its own: Joel Thomas Zimmerman is deadmau5, and also testpilot. In liquidslunk today, those are three artists with nothing between them. Alternate names don't fit: they're other ways of writing one artist's name, and what's credited with one is on that artist's page. This design adds aliases, as Discogs has them: artists of their own, each with its own page, releases and picture, grouped to say they're the same person.

It builds on the base design, [1_introduction.md](1_introduction.md). Here, "§4" and "phase 0" mean this document's own sections and phase, and "design §5.6" means the base design's. Other design docs cite this one as "5_artist_aliases.md §4" and "phase 0 of 5_artist_aliases.md".

## 1. Summary

- **An alias is another artist.** Two artists are aliases when they're the same person, or the same band, under two names: deadmau5 and testpilot. Each keeps its own page, releases, credits, picture, files and external library rules.
- **Aliases are groups, in one new table,** `artist_aliases`: each artist with aliases, and the number of its group. Every artist in a group is an alias of every other: Joel Thomas Zimmerman, deadmau5 and testpilot are one group of three (§4). Nothing else in the schema changes.
- **Linking two artists joins their groups.** An artist that's one of a person's names is an alias of all of them, so when testpilot already has aliases, linking deadmau5 with testpilot makes them deadmau5's too (§4.2).
- **They're shown on the artist page,** under its alternate names, and **edited in the artist form,** with the artist picker (§7).
- **Nothing else changes:** credits, the import, the lists, external libraries and the tags they write treat an alias as the artist it is (§6.3).
- **One migration, and one phase** (§8).

## 2. Scope

In scope:
- linking artists as aliases, and unlinking them, in the artist form;
- an artist's aliases on its page, each a link to its own page;
- what deleting an artist does to its group.

Out of scope:
- **Bands and their members,** Discogs's "Members" and "In Groups". A member is in a band, so those links have a direction, and more to say about each: when, and as what. They'd get a design and a table of their own (§10).
- **Label aliases.** Discogs has none. Its labels have parent labels and sublabels, which is another relation.
- **A discography with aliases.** Aliases are shown on the artist page, and nowhere else. An artist's Releases, Appearances and Credits tabs, and the release list's artist filter, stay that artist's own. Each alias's releases are on its own page, as on Discogs.
- **A name two people share** (§4.1).
- **Turning an alternate name into an alias, or back, and merging two artists.** Each moves links from one artist to another (§3).
- **Aliases in exported tags or external library rules** (§6.3).

## 3. Aliases and alternate names

liquidslunk already has alternate names: other spellings, initials, names in other scripts, and real names. An artist can be credited with any of them ([reference/database.md](../reference/database.md#credited-names)). An alias is something else:

| | Alternate name | Alias |
|---|---|---|
| What it is | Another way of writing the artist's name: "ALIKA" for Alika, "福居良" for Ryo Fukui | Another artist: the same person or band, under another name. testpilot, for deadmau5 |
| Its own page, picture and files | No: what's credited with it is on the artist's page, "as ALIKA" | Yes |
| Its own external library rule | No | Yes |
| Searching for it finds | The artist | The alias |
| On Discogs | A name variation (ANV), or the profile's real name | An alias |

- **Which one to use:** the same name in another form is an alternate name. Another name, for a project of its own, is an alias. That's Discogs's rule too.
- **A real name can be either.** Joel Thomas Zimmerman can be deadmau5's real name, an alternate name, with what's credited to it credited to deadmau5 "as Joel Thomas Zimmerman". Or it's an artist, and an alias, when its credits belong on a page of their own, as Discogs keeps songwriting credits under real names. liquidslunk doesn't decide which.
- **They don't mix.** An alias's alternate names are its own. A link to deadmau5 can't be credited with one of testpilot's names: the database already refuses a credited name that isn't the linked artist's.

## 4. Alias groups

Here, a group is an alias group: the artists that are one person or band. Bands themselves, and their members, are another relation (§2).

### 4.1 What a group is

- **Every artist in a group is an alias of every other one,** and an artist is in one group at most. Joel Thomas Zimmerman, deadmau5 and testpilot are one group, and each one's page shows the other two.
- **No artist is the main one.** Every name in a group is equal. A real name is an alias like the others, or an alternate name (§3).
- **A group has two artists or more.** An artist alone has no aliases, so it has no group.
- **A name two people share can't be an alias of both.** If two unrelated producers each released under one name, making it an alias of both would put them in one group, and make them aliases of each other. An alias means the same person, so the shared name is an alias of one of them, or of neither.

### 4.2 Linking and unlinking

- **Linking two artists joins their groups:**

  | Before | Linking deadmau5 with testpilot |
  |---|---|
  | Neither has aliases | A new group: deadmau5 and testpilot |
  | deadmau5 is in a group with Joel Thomas Zimmerman | testpilot joins it |
  | testpilot is in a group with Joel Thomas Zimmerman | deadmau5 joins it |
  | Each is in a group of its own | The two become one group, with everyone in both |

  The last row is why it's groups: an artist that's one of a person's names is an alias of all of them.
- **Unlinking takes one artist out of its group.** The others stay aliases of each other: taking testpilot out leaves Joel Thomas Zimmerman and deadmau5 together. When only one artist is left, the group goes.
- **Deleting an artist** takes it out of its group, the same way.
- **A mistaken link is undone artist by artist.** Linking deadmau5 with the wrong artist, which has two aliases of its own, brings all three into the group, and each one is taken out again. The form shows who a link brings, before it saves (§7.2).

## 5. The database

One migration, `add_artist_aliases`, adds one table and its trigger:

```sql
create table "artist_aliases" (
  "artist_id" integer not null primary key references "artists" ("id") on delete cascade,
  "alias_group" integer not null check ("alias_group" > 0)
) strict;
create index "artist_aliases_alias_group_index" on "artist_aliases" ("alias_group");
create trigger "artist_aliases_group_of_one_delete" after delete on "artist_aliases"
when (select count(*) from "artist_aliases" where "alias_group" = old."alias_group") = 1
begin
  delete from "artist_aliases" where "alias_group" = old."alias_group";
end;
```

- **One row per artist in a group,** keyed by the artist: its `INTEGER PRIMARY KEY` is the artist's id, so an artist is in one group at most, and can't be listed twice. An artist with no aliases has no row.
- **`alias_group` is a number, not a table.** A group has nothing of its own but its artists, so nothing refers to its number. A new group takes the highest number plus one, in the save's transaction, which holds the write lock. A number can come back once its group has gone, and that's harmless, since nothing kept it.
- **It's named `alias_group`, not `group_id`.** In music, a group is a band, which is what Discogs's Members and In Groups are about (§2). And every `_id` column in the schema refers to a table.
- **No group of one.** When an artist leaves a group and one is left, `artist_aliases_group_of_one_delete` deletes that one too, whether the row was deleted or its artist was (CASCADE fires it). Rows are only inserted and deleted, never updated (§6.1), so every way out of a group is a delete. The database can't refuse a group of one being inserted, since a new group's first row comes before its second. The app inserts a group whole, and a group of one would show no aliases anyway.
- **Reading** an artist's aliases is a join of the table with itself on `alias_group`: the artist's row, by its primary key, then its group, by the index. There's no recursion.
- **It's named for artists,** like `artist_alternate_names`, since labels have no aliases (§2).
- **No note, position or dates.** Discogs's aliases have none. Why an artist uses a name is about the artist, not about its group, and the page sorts aliases by name (§7.1). Bands, which would have more to say, get their own table (§2).
- **Rolling back** drops the table, and the groups with it, as rolling back `add_everything_rules` deletes everything rules.
- **The one-way door.** slunk already refuses these libraries, since `add_external_libraries`. A liquidslunk from before this migration refuses a library once it has run ("This library was upgraded by a newer version of liquidslunk."), apart from the backup the runner makes first. An existing library has no groups until the user links some.
- **`reference/database.md`** describes the table, its trigger and their rules.

## 6. The backend

### 6.1 The catalog

Aliases are part of the catalog context, with artists.
- **`repo/aliases.rs`:** an artist's aliases, and the rows of a group, inserted and deleted.
- **`catalog/aliases.rs`:** saving an artist's aliases, and reading them for its page.
- **They're saved with the artist** (`ArtistInput.aliases`), as its alternate names are, in the same transaction: a list of artist ids, which become the artist's group. Left out, they stay as they are.
  - **The save** deletes the rows of the artist and of the artists given, then inserts them all again under a new number, unless the list is empty. Everyone else keeps their row: the artist's old aliases that are left out stay aliases of each other, and so do the other aliases of an artist given without them. A group left with one artist loses it, through the trigger (§5).
  - **So the form decides who's in the group.** It lists the artist's aliases, and adds a picked artist's aliases with it (§7.2), so linking with testpilot joins testpilot's group, as §4.2 says.
  - An id given twice is kept once.
  - The artist itself is refused: "An artist can't be its own alias".
  - An unknown one is `NotFound`: "There's no artist 12".
  - A new artist can be created with its aliases.
- **They're read** for the page: the artist's aliases, by name ignoring case, then by id.
- **Deleting** an artist takes it out of its group, through the database's CASCADE and the trigger.

### 6.2 The commands

- `createArtist` and `updateArtist` take `aliases`, a list of artist ids.
- `artistHeader`, which every artist tab reads, and `artist`, which the form reads, return `aliases`: each one `{ id, name, nameLanguage }`.
- **`artistAliases`,** new: an artist's aliases, for the form, when it picks an artist (§7.2).

Every mutation already invalidates every query, so saving deadmau5's aliases refreshes testpilot's page too ([frontend.md](../development/frontend.md#commands)).

### 6.3 What doesn't change

- **Credits and credited names:** a link credits one artist, with that artist's own names (§3).
- **The import** matches artist tags by each artist's own names, so a testpilot tag finds testpilot.
- **Lists and search:** searching for "testpilot" finds testpilot, not deadmau5. The release list's artist filter is that artist alone (§2).
- **External libraries:** an artist rule includes that artist's releases, and not its aliases': a rule includes what it names, and an alias is one more rule. Exported tags name each artist as credited, as now.
- **The library check.**
- **The seeds** stay slunk's.

### 6.4 Tests

In `tests/catalog.rs`:
- **Saving:** a new group; joining one; two groups joined; leaving a group, with the others still together; leaving a group of two; an artist given without its own aliases; an empty list; left out; an id given twice; the artist itself; an unknown artist; and on create.
- **Reading:** an artist in a group, and one with none.
- **Deleting an artist,** from a group of three and from a group of two.
- **The table:** an artist in two groups is refused, and the trigger deletes a group's last artist, after a delete and after its artist is deleted.

The migration's `down` is checked with the others'.

## 7. The frontend

### 7.1 The artist page

- **Aliases** show under the alternate names, as a row of their own, in the same style: "Aliases: Joel Thomas Zimmerman, testpilot". Each links to its page, in its name's language (`textProps`). With none, there's no row.
- They're on every tab, since `artistHeader` has them.

### 7.2 The artist form

- **An Aliases field,** after Alternate names, in `/artists/create` and `/artists/:id/update`. It's a `ChipList`, as the tags editor's values are: the artist's aliases as chips, by name, each with a remove button, and the artist picker after them. There's no order to change, since aliases are sorted by name. The picker leaves out the artist and the aliases it has.
- **The picker can create an artist,** at once, as it does in the release form, so testpilot can be made from deadmau5's form. Cancel doesn't undo that.
- **Picking an artist brings its aliases.** Picking testpilot, whose alias is Joel Thomas Zimmerman, adds both, with a note: "With testpilot's alias, Joel Thomas Zimmerman". The form asks `artistAliases` for them. Each can still be removed before saving.
- **Removing an alias** takes it out of this artist's group, so it's no longer an alias of any of them.
- **Saving** is the form's Save, and asks nothing more: who a pick brings is in the field before it saves, and Cancel leaves the group as it was.
- **Its description:** "Other artists this one is: another name the same person or band releases under, with its own page, like testpilot for deadmau5. Each alias is an alias of the others too. Another way of writing this artist's name is an alternate name."

### 7.3 Deleting an artist

The delete dialog doesn't change. Aliases never block a delete, and the artist's aliases stay aliases of each other (§4.2).

## 8. Phase

Phase 0 ends with something usable and documented, and every check in AGENTS.md's "Before you finish" passes.

**Phase 0: Aliases**
- The migration (§5), and `reference/database.md`.
- The catalog's aliases (§6.1), with their tests (§6.4).
- `createArtist`, `updateArtist`, `artistHeader`, `artist` and `artistAliases` (§6.2), and `src/bindings.ts` with them.
- The artist page and the form (§7), checked in screenshots.
- Docs: aliases in `backend.md`'s catalog context, and the artist page and form in `frontend.md`.
- A release build for the user to try. Then mark this design complete.
- *Exit:*
  - In a scratch library, Joel Thomas Zimmerman, deadmau5 and testpilot, in one group, show each other on their pages.
  - Linking an artist with one that has aliases of its own joins their groups. Unlinking an artist, and deleting one, leave the others together.
  - The migration applies, rolls back and applies again through the CLI.

## 9. Risks and checks

There are few: nothing touches files, and nothing in slunk's tables changes.
- **Joining groups is quiet.** Linking two artists that each have aliases makes everyone in both aliases of each other. The form lists who a link brings before it saves (§7.2), and a mistake is undone artist by artist (§4.2).
- **A group of one can be inserted,** since the database can't refuse it (§5). The save's tests check that the app never leaves one.

## 10. Alternatives considered

- **Links between pairs,** this design's first draft: `artist_aliases (artist_id, alias_id)`, one row per pair, the lower id first. Each link is exactly what the user said, so a name two people share could be an alias of both. But "the same person" had to be read through the links, with a recursive query, and the page had to say how: "testpilot (through Joel Thomas Zimmerman)". Taking out the artist the others were linked through would split them. A group holds that meaning by itself.
- **Direct links only, as Discogs shows them:** pairs, with each page showing only its own links. It's exact, but every pair is linked by hand: ten links for five names.
- **A groups table,** with `alias_group_id` referring to it. SQLite would number the groups, and a group would have a place for something of its own later. It has nothing now, and its rows would need deleting as groups empty, on top of the groups of one.
- **A column on `artists`:** its alias group. It saves a table, but changes slunk's, and every artist without aliases would carry a NULL.
- **One table for every relation between artists,** aliases and band members, with a `kind`. Members have a direction and more to say (§2), so the table would need nullable columns, and a CHECK per kind. Each relation gets its own table.
- **Alternate names marked as projects of their own.** One page for both names is what an alias isn't: its own releases, picture and rules are what make it worth keeping apart (§3).

## 11. Settled questions

The first draft left these open. They were settled on 2026-10-26:
1. **Groups, not links.** The first draft linked pairs, and read aliases through the links (§10). An alias means the same person, so each of a person's names is an alias of all the others, which a group holds without a recursive query or "through". A name two people share isn't an alias of both (§4.1).
2. **No discography with aliases.** Aliases are shown on the artist page, and nowhere else (§2).
3. **The table is `artist_aliases`,** like `artist_alternate_names` (§5).
