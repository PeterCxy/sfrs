table! {
    items (id) {
        id -> Integer,
        owner -> Integer,
        uuid -> Text,
        content -> Nullable<Text>,
        content_type -> Text,
        enc_item_key -> Nullable<Text>,
        deleted -> Bool,
        created_at -> Date,
        updated_at -> Date,
    }
}

table! {
    users (id) {
        id -> Integer,
        email -> Text,
        password -> Text,
        pw_cost -> Text,
        pw_nonce -> Text,
        version -> Text,
    }
}

joinable!(items -> users (owner));

allow_tables_to_appear_in_same_query!(
    items,
    users,
);
