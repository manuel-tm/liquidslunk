-- Credit lines, each inserted before its artists. The generator runs a seed in
-- one transaction, so the deferred key finds each line's first artist.
INSERT INTO release_credits (id, release_id, role, position) VALUES
  (1, 1, 'Producer', 0),
  (2, 2, 'Mixed By', 0);
INSERT INTO artists_release_credits (id, artist_id, credit_id, position) VALUES
  (1, 3, 1, 0),
  (2, 1, 1, 1),
  (3, 2, 2, 0);
INSERT INTO track_credits (id, track_id, role, position) VALUES (1, 1, 'Composed By', 0);
INSERT INTO artists_track_credits (id, artist_id, credit_id, position) VALUES (1, 1, 1, 0);
