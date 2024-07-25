use core::fmt;
use core::hash::Hash;

use super::Bag;

impl<K, V> serde::ser::Serialize for Bag<K, V>
where
    K: serde::ser::Serialize,
    V: serde::ser::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::ser::Serialize::serialize(&self.0, serializer)
    }
}

impl<'de, K, V> serde::Deserialize<'de> for Bag<K, V>
where
    K: serde::Deserialize<'de> + Eq + Hash,
    V: serde::Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Bag<K, V>, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        struct SeqVisitor<K, V>(std::marker::PhantomData<(K, V)>);

        impl<'de, K, V> serde::de::Visitor<'de> for SeqVisitor<K, V>
        where
            K: serde::Deserialize<'de> + Eq + Hash,
            V: serde::Deserialize<'de>,
        {
            type Value = Bag<K, V>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "a map of sequences")
            }

            fn visit_map<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut map = indexmap::IndexMap::new();
                while let Some((key, value)) = seq.next_entry()? {
                    if let Some(_) = map.insert(key, value) {
                        return Err(serde::de::Error::duplicate_field("key"));
                    }
                }
                Ok(Bag(map))
            }
        }

        deserializer.deserialize_map(SeqVisitor(std::marker::PhantomData))
    }
}

#[cfg(test)]
mod test {
    use crate::Bag;

    #[test]
    fn serde_roundtrip() -> serde_json::Result<()> {
        let bag = Bag::from_iter([("foo", 1), ("foo", 2), ("bar", 3)]);
        println!("{:?}", bag);
        let bag = serde_json::to_string(&bag)?;
        println!("{}", bag);
        let bag = serde_json::from_str::<Bag<String, i32>>(&bag)?;
        println!("{:?}", bag);
        Ok(())
    }

    #[test]
    #[should_panic]
    fn invalid_map() {
        let _: Bag<i32, i32> = serde_json::from_str(
            r#"
            {
                "1": "3"
            }
        "#,
        )
        .unwrap();
    }
}
