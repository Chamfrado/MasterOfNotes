-- Migration 003 (SPEC-012 R3): voice level (dB, vocals/mix RMS) measured when a reference is extracted
-- from the video, so the weak-voice warning survives leaving the screen. NULL for any other source.
-- Never edit after integration.
ALTER TABLE melody_references ADD COLUMN voice_level_db REAL;
