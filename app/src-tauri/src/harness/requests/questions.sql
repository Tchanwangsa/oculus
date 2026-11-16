-- Answers stay with their request when the live callback becomes a message.
ALTER TABLE harness_requests ADD COLUMN answers TEXT;
