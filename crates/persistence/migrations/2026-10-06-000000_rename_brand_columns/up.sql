-- The app was renamed to Leanterm; the AI panel width column belonged to a removed feature.
ALTER TABLE windows DROP COLUMN warp_ai_width;
ALTER TABLE windows RENAME COLUMN warp_drive_index_width TO leanterm_drive_index_width;
