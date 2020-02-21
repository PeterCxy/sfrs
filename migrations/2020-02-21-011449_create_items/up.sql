CREATE TABLE items (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    owner INTEGER NOT NULL,
    uuid VARCHAR NOT NULL,
    content VARCHAR,
    content_type VARCHAR NOT NULL,
    enc_item_key VARCHAR,
    deleted BOOLEAN NOT NULL,
    created_at VARCHAR NOT NULL,
    updated_at VARCHAR,
    FOREIGN KEY (owner)
        REFERENCES users (id)
)