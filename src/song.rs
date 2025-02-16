use anyhow::Result;
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use gtk::{
    glib::{self, once_cell::sync::Lazy},
    prelude::*,
    subclass::prelude::*,
};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use std::{
    cell::{Cell, OnceCell, RefCell},
    rc::Rc,
};

use crate::{
    album_art::AlbumArt,
    date_time::DateTime,
    external_links::{ExternalLinkKey, ExternalLinks},
    serde_helpers,
    uid::Uid,
    Application,
};

mod imp {
    use super::*;

    #[derive(Default, glib::Properties, Serialize, Deserialize)]
    #[properties(wrapper_type = super::Song)]
    pub struct Song {
        /// Unique ID
        #[property(get, set, construct_only)]
        #[serde(with = "serde_helpers::once_cell")]
        pub(super) id: OnceCell<Uid>,
        /// Title of the song
        #[property(get, set, construct_only)]
        pub(super) title: RefCell<String>,
        /// Artist of the song
        #[property(get, set, construct_only)]
        pub(super) artist: RefCell<String>,
        /// Album where the song was from
        #[property(get, set, construct_only)]
        pub(super) album: RefCell<String>,
        /// Arbitrary string for release date
        #[property(get, set, construct_only)]
        pub(super) release_date: RefCell<Option<String>>,
        /// Links relevant to the song
        #[property(get, set, construct_only)]
        pub(super) external_links: RefCell<ExternalLinks>,
        /// Link where the album art can be downloaded
        #[property(get, set, construct_only)]
        pub(super) album_art_link: RefCell<Option<String>>,
        /// Link to a sample of the song
        #[property(get, set, construct_only)]
        pub(super) playback_link: RefCell<Option<String>>,
        /// Lyrics of the song
        #[property(get, set, construct_only)]
        pub(super) lyrics: RefCell<Option<String>>,
        /// Date and time when last heard
        #[property(get, set = Self::set_last_heard, explicit_notify)]
        pub(super) last_heard: RefCell<Option<DateTime>>,
        /// Whether the song was heard for the first time
        #[property(get, set = Self::set_is_newly_heard, explicit_notify)]
        pub(super) is_newly_heard: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Song {
        const NAME: &'static str = "MsaiSong";
        type Type = super::Song;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Song {}

    impl Song {
        fn set_last_heard(&self, last_heard: Option<DateTime>) {
            let obj = self.obj();

            if last_heard == obj.last_heard() {
                return;
            }

            self.last_heard.replace(last_heard);
            obj.notify_last_heard();
        }

        fn set_is_newly_heard(&self, is_newly_heard: bool) {
            let obj = self.obj();

            if is_newly_heard == obj.is_newly_heard() {
                return;
            }

            self.is_newly_heard.set(is_newly_heard);
            obj.notify_is_newly_heard();
        }
    }
}

glib::wrapper! {
    pub struct Song(ObjectSubclass<imp::Song>);
}

impl Song {
    pub const NONE: Option<&'static Self> = None;

    /// The parameter `Uid` must be unique to each [`Song`] so that [`crate::model::SongList`] will
    /// treat them different.
    ///
    /// The last heard will be the `DateTime` when this is constructed
    pub fn builder(id: &Uid, title: &str, artist: &str, album: &str) -> SongBuilder {
        SongBuilder::new(id, title, artist, album)
    }

    /// Returns the score of song against the pattern.
    pub fn fuzzy_match(&self, pattern: &str) -> Option<i64> {
        static FUZZY_MATCHER: Lazy<SkimMatcherV2> = Lazy::new(SkimMatcherV2::default);

        let choice = format!("{} {}", self.artist(), self.title());
        FUZZY_MATCHER.fuzzy_match(&choice, pattern)
    }

    /// String copied to clipboard when copying self.
    pub fn copy_term(&self) -> String {
        format!("{} - {}", self.artist(), self.title())
    }

    /// Get a reference to the Uid instead of cloning it like in `Self::id()`
    pub fn id_ref(&self) -> &Uid {
        self.imp().id.get().unwrap()
    }

    /// Returns a result of album art for the corresponding album art link if it exists
    pub fn album_art(&self) -> Option<Rc<AlbumArt>> {
        let album_art_link = self.album_art_link()?;

        Some(
            Application::get()
                .album_art_store()
                .get_or_init(&album_art_link),
        )
    }
}

impl Serialize for Song {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.imp().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Song {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let deserialized_imp = imp::Song::deserialize(deserializer)?;
        Ok(glib::Object::builder()
            .property(
                "id",
                deserialized_imp
                    .id
                    .into_inner()
                    .ok_or_else(|| de::Error::missing_field("id"))?,
            )
            .property("title", deserialized_imp.title.into_inner())
            .property("artist", deserialized_imp.artist.into_inner())
            .property("album", deserialized_imp.album.into_inner())
            .property("release-date", deserialized_imp.release_date.into_inner())
            .property(
                "external-links",
                deserialized_imp.external_links.into_inner(),
            )
            .property(
                "album-art-link",
                deserialized_imp.album_art_link.into_inner(),
            )
            .property("playback-link", deserialized_imp.playback_link.into_inner())
            .property("lyrics", deserialized_imp.lyrics.into_inner())
            .property("last-heard", deserialized_imp.last_heard.into_inner())
            .property(
                "is-newly-heard",
                deserialized_imp.is_newly_heard.into_inner(),
            )
            .build())
    }
}

#[must_use = "builder doesn't do anything unless built"]
pub struct SongBuilder {
    properties: Vec<(&'static str, glib::Value)>,
    external_links: ExternalLinks,
}

impl SongBuilder {
    fn new(id: &Uid, title: &str, artist: &str, album: &str) -> Self {
        Self {
            properties: vec![
                ("id", id.into()),
                ("title", title.into()),
                ("artist", artist.into()),
                ("album", album.into()),
            ],
            external_links: ExternalLinks::default(),
        }
    }

    pub fn newly_heard(&mut self, value: bool) -> &mut Self {
        self.properties.push(("is-newly-heard", value.into()));
        self
    }

    pub fn release_date(&mut self, value: &str) -> &mut Self {
        self.properties.push(("release-date", value.into()));
        self
    }

    pub fn album_art_link(&mut self, value: &str) -> &mut Self {
        self.properties.push(("album-art-link", value.into()));
        self
    }

    pub fn playback_link(&mut self, value: &str) -> &mut Self {
        self.properties.push(("playback-link", value.into()));
        self
    }

    pub fn lyrics(&mut self, value: &str) -> &mut Self {
        self.properties.push(("lyrics", value.into()));
        self
    }

    /// Pushes an external link. This is not idempotent.
    pub fn external_link(&mut self, key: ExternalLinkKey, value: impl Into<String>) -> &mut Self {
        self.external_links.insert(key, value.into());
        self
    }

    pub fn build(&mut self) -> Song {
        self.properties
            .push(("external-links", self.external_links.to_value()));
        glib::Object::with_mut_values(Song::static_type(), &mut self.properties)
            .downcast()
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use crate::external_link::ExternalLink;

    #[test]
    fn id_ref() {
        let song = Song::builder(
            &Uid::from("UniqueSongId"),
            "Some song",
            "Someone",
            "SomeAlbum",
        )
        .build();
        assert_eq!(&song.id(), song.id_ref());
    }

    #[test]
    fn properties() {
        let song = Song::builder(
            &Uid::from("UniqueSongId"),
            "Some song",
            "Someone",
            "SomeAlbum",
        )
        .release_date("00-00-0000")
        .album_art_link("https://album.png")
        .playback_link("https://test.mp3")
        .lyrics("Some song lyrics")
        .newly_heard(true)
        .build();

        assert_eq!(song.title(), "Some song");
        assert_eq!(song.artist(), "Someone");
        assert_eq!(song.album(), "SomeAlbum");
        assert_eq!(song.release_date().as_deref(), Some("00-00-0000"));
        assert_eq!(song.album_art_link().as_deref(), Some("https://album.png"));
        assert_eq!(song.playback_link().as_deref(), Some("https://test.mp3"));
        assert_eq!(song.lyrics().as_deref(), Some("Some song lyrics"));
        assert!(song.is_newly_heard());
    }

    fn assert_song_eq(v1: &Song, v2: &Song) {
        assert_eq!(v1.id_ref(), v2.id_ref());
        assert_eq!(v1.title(), v2.title());
        assert_eq!(v1.artist(), v2.artist());
        assert_eq!(v1.album(), v2.album());
        assert_eq!(v1.release_date(), v2.release_date());

        assert_eq!(v1.external_links().n_items(), v2.external_links().n_items());
        for (v1_item, v2_item) in v1
            .external_links()
            .iter::<ExternalLink>()
            .zip(v2.external_links().iter::<ExternalLink>())
        {
            let v1_item = v1_item.unwrap();
            let v2_item = v2_item.unwrap();
            assert_eq!(v1_item.key(), v2_item.key());
            assert_eq!(v1_item.value(), v2_item.value());
        }

        assert_eq!(v1.album_art_link(), v2.album_art_link());
        assert_eq!(v1.playback_link(), v2.playback_link());
        assert_eq!(v1.lyrics(), v2.lyrics());
        assert_eq!(v1.last_heard(), v2.last_heard());
        assert_eq!(v1.is_newly_heard(), v2.is_newly_heard());
    }

    #[test]
    fn serde_bincode() {
        let val = SongBuilder::new(&Uid::from("a"), "A Title", "A Artist", "A Album").build();
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Song>(&bytes).unwrap();
        assert_song_eq(&val, &de_val);

        let val = SongBuilder::new(&Uid::from("b"), "B Title", "B Artist", "B Album")
            .newly_heard(true)
            .build();
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Song>(&bytes).unwrap();
        assert_song_eq(&val, &de_val);

        let val = SongBuilder::new(&Uid::from("c"), "C Title", "C Artist", "C Album")
            .release_date("some value")
            .album_art_link("some value")
            .playback_link("some value")
            .lyrics("some value")
            .build();
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Song>(&bytes).unwrap();
        assert_song_eq(&val, &de_val);

        let val = SongBuilder::new(&Uid::from("d"), "D Title", "D Artist", "D Album").build();
        val.set_last_heard(DateTime::now_utc());
        let bytes = bincode::serialize(&val).unwrap();
        let de_val = bincode::deserialize::<Song>(&bytes).unwrap();
        assert_song_eq(&val, &de_val);
    }

    #[test]
    fn deserialize() {
        let song: Song = serde_json::from_str(
            r#"{
                "id": "UniqueSongId",
                "title": "Some song",
                "artist": "Someone",
                "album": "SomeAlbum",
                "release_date": "00-00-0000",
                "external_links": {},
                "album_art_link": "https://album.png",
                "playback_link": "https://test.mp3",
                "lyrics": "Some song lyrics",
                "last_heard": "2022-05-14T10:15:37.798479+08",
                "is_newly_heard": true
            }"#,
        )
        .unwrap();

        assert_eq!(song.id_ref(), &Uid::from("UniqueSongId"));
        assert_eq!(song.title(), "Some song");
        assert_eq!(song.artist(), "Someone");
        assert_eq!(song.album(), "SomeAlbum");
        assert_eq!(song.release_date().as_deref(), Some("00-00-0000"));
        assert_eq!(song.external_links().n_items(), 0);
        assert_eq!(song.album_art_link().as_deref(), Some("https://album.png"));
        assert_eq!(song.playback_link().as_deref(), Some("https://test.mp3"));
        assert_eq!(song.lyrics().as_deref(), Some("Some song lyrics"));
        assert_eq!(
            song.last_heard().unwrap().format_iso8601(),
            "2022-05-14T10:15:37.798479+08"
        );
        assert!(song.is_newly_heard());
    }
}
