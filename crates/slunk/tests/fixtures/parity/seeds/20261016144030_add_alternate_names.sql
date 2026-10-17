INSERT INTO artist_alternate_names (id, artist_id, name, language, real_name, note, position) VALUES
  (1, 2, 'ALIKA', NULL, 0, NULL, 0),
  (2, 2, 'Alika Milova', 'et', 1, 'Birth name', 1),
  (3, 1, '福居良', 'ja', 0, NULL, 0);
INSERT INTO label_alternate_names (id, label_id, name, language, note, position) VALUES
  (1, 1, 'トリオ・レコード', 'ja', NULL, 0);
UPDATE artists SET name_language = 'ja-Latn' WHERE id = 1;
UPDATE labels SET name_language = 'en' WHERE id = 1;
