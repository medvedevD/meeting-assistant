-- Phase 3: RegenerateProtocol jobs carry the template to use.
ALTER TABLE jobs ADD COLUMN template_name TEXT;
