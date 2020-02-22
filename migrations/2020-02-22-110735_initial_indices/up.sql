CREATE INDEX index_user_email_20200222110735 ON users(email);
CREATE INDEX index_item_uuid_owner_20200222110735 ON items(uuid, owner);
CREATE INDEX index_token_uid_20200222110735 ON tokens(uid);