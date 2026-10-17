-- Rows for every table the baseline creates. The last file is deleted, so the
-- files counter stays ahead of the highest id.
INSERT INTO files (id, path, type, size_bytes, mtime, sha256, inserted_at, updated_at) VALUES
  (1, 'Scenery/01 It Could Happen to You.flac', 'music', 31457280, 1700000000, '0000000000000000000000000000000000000000000000000000000000000001', 1700000100, 1700000100),
  (2, 'Scenery/02 Early Summer.flac', 'music', 29360128, 1700000001, '0000000000000000000000000000000000000000000000000000000000000002', 1700000100, 1700000100),
  (3, 'Scenery/cover.jpg', 'image', 524288, 1700000002, '0000000000000000000000000000000000000000000000000000000000000003', 1700000100, 1700000100),
  (4, 'Virmalised/01 Intro.flac', 'music', 10485760, 1700000003, '0000000000000000000000000000000000000000000000000000000000000004', 1700000100, 1700000100),
  (5, 'Virmalised/CD2/01 Virmalised.flac', 'music', 20971520, 1700000004, '0000000000000000000000000000000000000000000000000000000000000005', 1700000100, 1700000100),
  (6, 'Artists/alika.jpg', 'image', 262144, 1700000005, '0000000000000000000000000000000000000000000000000000000000000006', 1700000100, 1700000100),
  (7, '../Other/notes.txt', 'other', 1024, 1700000006, '0000000000000000000000000000000000000000000000000000000000000007', 1700000100, 1700000100),
  (8, 'Scenery/back.jpg', 'image', 131072, 1700000007, '0000000000000000000000000000000000000000000000000000000000000008', 1700000100, 1700000100);
DELETE FROM files WHERE id = 8;

INSERT INTO artists (id, name, picture, inserted_at, updated_at) VALUES
  (1, 'Ryo Fukui', NULL, 1700000200, 1700000200),
  (2, 'Alika', 6, 1700000200, 1700000300),
  (3, 'Sam Jones', NULL, 1700000200, 1700000200);
INSERT INTO labels (id, name, picture, inserted_at, updated_at) VALUES
  (1, 'Trio Records', NULL, 1700000200, 1700000200),
  (2, 'Self-released', NULL, 1700000200, 1700000200);

-- 1976, 2018-05, and no date
INSERT INTO releases (id, title, release_date, release_date_precision, catalog_number, cover_picture, various_artists, inserted_at, updated_at) VALUES
  (1, 'Scenery', 189302400, 'year', 'PAP-9019', 3, 0, 1700000400, 1700000400),
  (2, 'Virmalised', 1525132800, 'month', NULL, NULL, 0, 1700000400, 1700000400),
  (3, 'Jazz Sampler', NULL, NULL, NULL, NULL, 1, 1700000400, 1700000400);
INSERT INTO tracks (id, title, duration_ms, track_number, cd_number, release_id, file_id, inserted_at, updated_at) VALUES
  (1, 'It Could Happen to You', 452000, 1, 1, 1, 1, 1700000500, 1700000500),
  (2, 'Early Summer', 431000, 2, 1, 1, 2, 1700000500, 1700000500),
  (3, 'Intro', 120000, 1, 1, 2, 4, 1700000500, 1700000500),
  (4, 'Virmalised', NULL, 1, 2, 2, 5, 1700000500, 1700000500);

INSERT INTO artists_releases (id, artist_id, release_id, position) VALUES
  (1, 1, 1, 0),
  (2, 2, 2, 0);
INSERT INTO artists_tracks (id, artist_id, track_id, featured, position) VALUES
  (1, 3, 1, 1, 0),
  (2, 2, 3, 0, 0);
INSERT INTO labels_releases (id, label_id, release_id, position) VALUES
  (1, 1, 1, 0),
  (2, 2, 2, 0);

-- The baseline's credits, by role: producer and composer are starter roles
INSERT INTO release_credits (id, release_id, artist_id, role_id, position)
  SELECT 1, 1, 3, id, 0 FROM credit_roles WHERE name = 'producer';
INSERT INTO track_credits (id, track_id, artist_id, role_id, position)
  SELECT 1, 1, 1, id, 0 FROM credit_roles WHERE name = 'composer';

INSERT INTO release_formats_releases (id, format_id, release_id)
  SELECT 1, id, 1 FROM release_formats WHERE name = 'Album';
INSERT INTO release_formats_releases (id, format_id, release_id)
  SELECT 2, id, 2 FROM release_formats WHERE name = 'EP';
INSERT INTO release_media_releases (id, medium_id, release_id)
  SELECT 1, id, 1 FROM release_media WHERE name = 'CD';
INSERT INTO release_media_releases (id, medium_id, release_id)
  SELECT 2, id, 2 FROM release_media WHERE name = 'Digital';

INSERT INTO files_releases (id, release_id, file_id, note) VALUES (1, 1, 7, 'Liner notes');
INSERT INTO artists_files (id, artist_id, file_id, note) VALUES (1, 2, 6, NULL);
INSERT INTO files_tracks (id, track_id, file_id, note) VALUES (1, 1, 3, 'Back cover');

INSERT INTO text_tags (id, name, multiple) VALUES (1, 'Genre', 1);
INSERT INTO real_number_tags (id, name, multiple) VALUES (1, 'Score', 0);
INSERT INTO date_tags (id, name, multiple) VALUES (1, 'Purchased', 0);
INSERT INTO release_text_tags (id, release_id, tag_id, value) VALUES
  (1, 1, 1, 'Jazz'),
  (2, 1, 1, 'Bebop');
INSERT INTO release_real_number_tags (id, release_id, tag_id, value) VALUES (1, 1, 1, 9.5);
-- 2020-09-13, a day, which the next migration checks
INSERT INTO release_date_tags (id, release_id, tag_id, value) VALUES (1, 2, 1, 1599955200);
