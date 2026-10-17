-- Alika credited as "ALIKA", and Ryo Fukui as "福居良"
UPDATE artists_releases SET credited_name_id = 1 WHERE id = 2;
UPDATE artists_tracks SET credited_name_id = 1 WHERE id = 2;
UPDATE artists_release_credits SET credited_name_id = 1 WHERE id = 3;
UPDATE artists_track_credits SET credited_name_id = 3 WHERE id = 1;
