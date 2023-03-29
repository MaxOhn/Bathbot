use rkyv::{
    with::{ArchiveWith, DeserializeWith, SerializeWith},
    Archive, Fallible,
};
use twilight_model::channel::ChannelType;

pub struct ChannelTypeRkyv;

impl ArchiveWith<ChannelType> for ChannelTypeRkyv {
    type Archived = u8;
    type Resolver = ();

    #[inline]
    unsafe fn resolve_with(
        field: &ChannelType,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        let byte = match *field {
            ChannelType::GuildText => 0,
            ChannelType::Private => 1,
            ChannelType::GuildVoice => 2,
            ChannelType::Group => 3,
            ChannelType::GuildCategory => 4,
            ChannelType::GuildAnnouncement => 5,
            ChannelType::AnnouncementThread => 6,
            ChannelType::PublicThread => 7,
            ChannelType::PrivateThread => 8,
            ChannelType::GuildStageVoice => 9,
            ChannelType::GuildDirectory => 10,
            ChannelType::GuildForum => 11,
            ChannelType::Unknown(other) => other,
            _ => 255,
        };

        <u8 as Archive>::resolve(&byte, pos, resolver, out);
    }
}

impl<S: Fallible + ?Sized> SerializeWith<ChannelType, S> for ChannelTypeRkyv {
    #[inline]
    fn serialize_with(
        _: &ChannelType,
        _: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        Ok(())
    }
}

impl<D: Fallible + ?Sized> DeserializeWith<u8, ChannelType, D> for ChannelTypeRkyv {
    #[inline]
    fn deserialize_with(byte: &u8, _: &mut D) -> Result<ChannelType, <D as Fallible>::Error> {
        let channel_type = match *byte {
            0 => ChannelType::GuildText,
            1 => ChannelType::Private,
            2 => ChannelType::GuildVoice,
            3 => ChannelType::Group,
            4 => ChannelType::GuildCategory,
            5 => ChannelType::GuildAnnouncement,
            6 => ChannelType::AnnouncementThread,
            7 => ChannelType::PublicThread,
            8 => ChannelType::PrivateThread,
            9 => ChannelType::GuildStageVoice,
            10 => ChannelType::GuildDirectory,
            11 => ChannelType::GuildForum,
            other => ChannelType::Unknown(other),
        };

        Ok(channel_type)
    }
}
