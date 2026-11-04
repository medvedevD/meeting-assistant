-- Tokens the LLM reported for the stored protocol, shown under it in the UI.
-- NULL when the protocol predates this column or the provider sent no usage.
-- Cleared together with protocol_text, so they always describe the current one.
ALTER TABLE meetings ADD COLUMN protocol_input_tokens INTEGER;
ALTER TABLE meetings ADD COLUMN protocol_output_tokens INTEGER;
