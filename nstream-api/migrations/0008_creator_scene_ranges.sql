ALTER TABLE jobs ADD COLUMN scene_ranges JSONB NOT NULL DEFAULT '[]'::jsonb;
