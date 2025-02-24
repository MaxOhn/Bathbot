use rkyv::{
    Archive, Archived, Deserialize, Place, Resolver, Serialize,
    rancor::Fallible,
    with::{ArchiveWith, DeserializeWith, SerializeWith},
};

pub struct UnwrapOrDefault;

impl<T: Archive + Default> ArchiveWith<Option<T>> for UnwrapOrDefault {
    type Archived = Archived<T>;
    type Resolver = Resolver<T>;

    #[inline]
    fn resolve_with(field: &Option<T>, resolver: Self::Resolver, out: Place<Self::Archived>) {
        match field {
            Some(value) => Archive::resolve(value, resolver, out),
            None => Archive::resolve(&T::default(), resolver, out),
        }
    }
}

impl<T, S> SerializeWith<Option<T>, S> for UnwrapOrDefault
where
    T: Archive + Default + Serialize<S>,
    S: Fallible + ?Sized,
{
    #[inline]
    fn serialize_with(field: &Option<T>, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        match field {
            Some(value) => Serialize::serialize(value, serializer),
            None => Serialize::serialize(&T::default(), serializer),
        }
    }
}

impl<T, D> DeserializeWith<Archived<T>, T, D> for UnwrapOrDefault
where
    T: Archive,
    Archived<T>: Deserialize<T, D>,
    D: Fallible + ?Sized,
{
    #[inline]
    fn deserialize_with(field: &Archived<T>, deserializer: &mut D) -> Result<T, D::Error> {
        field.deserialize(deserializer)
    }
}
