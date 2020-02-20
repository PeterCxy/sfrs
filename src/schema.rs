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
