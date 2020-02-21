table! {
    items (id) {
        id -> BigInt, // Forced, diesel does not support intepreting Integer as i64
        owner -> Integer,
        uuid -> Text,
        content -> Nullable<Text>,
        content_type -> Text,
        enc_item_key -> Nullable<Text>,
        deleted -> Bool,
        created_at -> Timestamp,
        updated_at -> Nullable<Timestamp>,
    }
}

table! {
    users (id) {
        id -> Integer,
        uuid -> Text,
        email -> Text,
        password -> Text,
        pw_cost -> Integer,
        pw_nonce -> Text,
        version -> Text,
    }
}

joinable!(items -> users (owner));

allow_tables_to_appear_in_same_query!(
    items,
    users,
);
