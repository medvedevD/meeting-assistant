-- Phase 4: persist the classified terminal failure cause (UI error banners).
-- Live stage/percent are kept in memory only and never stored.
ALTER TABLE jobs ADD COLUMN error_class TEXT;
