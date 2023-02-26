use anyhow::{anyhow, Result};
use gtk::{glib, prelude::*, subclass::prelude::*};
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use crate::{
    core::{AlbumArt, DateTime},
    model::{db, ExternalLinkKey, ExternalLinks, SongId},
    utils,
};

pub fn serialize_once_cell<S>(
    cell: &OnceCell<impl Serialize>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    cell.get().serialize(serializer)
}

pub fn deserialize_once_cell<'de, D, T>(deserializer: D) -> Result<OnceCell<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(OnceCell::with_value(T::deserialize(deserializer)?))
}

mod imp {
    use super::*;

    #[derive(Debug, Default, glib::Properties, Serialize, Deserialize)]
    #[properties(wrapper_type = super::Song)]
    #[serde(default)]
    pub struct Song {
        /// Unique ID
        #[property(get, set, construct_only)]
        #[serde(
            serialize_with = "serialize_once_cell",
            deserialize_with = "deserialize_once_cell"
        )]
        pub(super) id: OnceCell<SongId>,
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
        pub(super) last_heard: RefCell<DateTime>,
        /// Whether the song was heard for the first time
        #[property(get, set = Self::set_is_newly_heard, explicit_notify)]
        pub(super) is_newly_heard: Cell<bool>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Song {
        const NAME: &'static str = "MsaiSong";
        type Type = super::Song;
    }

    impl ObjectImpl for Song {
        crate::derived_properties!();
    }

    impl Song {
        fn set_last_heard(&self, last_heard: DateTime) {
            let obj = self.obj();

            if last_heard == obj.last_heard() {
                return;
            }

            db::song::update_last_heard(&obj.id(), last_heard.clone()).unwrap();
            self.last_heard.replace(last_heard);
            obj.notify_last_heard();
        }

        fn set_is_newly_heard(&self, is_newly_heard: bool) {
            let obj = self.obj();

            if is_newly_heard == obj.is_newly_heard() {
                return;
            }

            db::song::update_is_newly_heard(&obj.id(), is_newly_heard).unwrap();
            self.is_newly_heard.set(is_newly_heard);
            obj.notify_is_newly_heard();
        }
    }
}

glib::wrapper! {
    pub struct Song(ObjectSubclass<imp::Song>);
}

impl Song {
    /// The parameter `SongID` must be unique to each [`Song`] so that [`crate::model::SongList`] will
    /// treat them different.
    ///
    /// The last heard will be the `DateTime` when this is constructed
    pub fn builder(id: &SongId, title: &str, artist: &str, album: &str) -> SongBuilder {
        SongBuilder::new(id, title, artist, album)
    }

    /// String to match to when searching for self.
    pub fn search_term(&self) -> String {
        format!("{}{}", self.title(), self.artist())
    }

    /// String copied to clipboard when copying self.
    pub fn copy_term(&self) -> String {
        format!("{} - {}", self.artist(), self.title())
    }

    pub fn album_art(&self) -> Result<Rc<AlbumArt>> {
        let album_art_link = self
            .album_art_link()
            .ok_or_else(|| anyhow!("Song doesn't have an album art link"))?;

        utils::app_instance()
            .album_art_store()?
            .get_or_init(&album_art_link)
    }
}

impl TryFrom<&rusqlite::Row<'_>> for Song {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row<'_>) -> Result<Self, rusqlite::Error> {
        let this = glib::Object::builder::<Self>()
            .property("id", row.get::<_, SongId>(0)?)
            .property("title", row.get::<_, String>(1)?)
            .property("artist", row.get::<_, String>(2)?)
            .property("album", row.get::<_, String>(3)?)
            .property("release-date", row.get::<_, Option<String>>(4)?)
            .property("external-links", row.get::<_, ExternalLinks>(5)?)
            .property("album-art-link", row.get::<_, Option<String>>(6)?)
            .property("playback-link", row.get::<_, Option<String>>(7)?)
            .property("lyrics", row.get::<_, Option<String>>(8)?)
            .build();

        // Avoid calling the setters which will unnecessarily update the database
        let imp = this.imp();
        imp.last_heard.replace(row.get::<_, DateTime>(9)?);
        imp.is_newly_heard.replace(row.get::<_, bool>(10)?);

        Ok(this)
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
            .property("id", deserialized_imp.id.into_inner().unwrap_or_default())
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
    pub fn new(id: &SongId, title: &str, artist: &str, album: &str) -> Self {
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

    #[test]
    fn properties() {
        let song = Song::builder(
            &SongId::new_for_test("UniqueSongId"),
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

    #[test]
    fn deserialize() {
        let song: Song = serde_json::from_str(
            r#"{
                "id": "Test-UniqueSongId",
                "last_heard": "2022-05-14T10:15:37.798479+08",
                "title": "Some song",
                "artist": "Someone",
                "album": "SomeAlbum",
                "release_date": "00-00-0000",
                "external_links": {},
                "album_art_link": "https://album.png",
                "playback_link": "https://test.mp3",
                "lyrics": "Some song lyrics"
            }"#,
        )
        .unwrap();

        assert_eq!(song.id(), SongId::new_for_test("UniqueSongId"));
        assert_eq!(
            song.last_heard().to_iso8601(),
            "2022-05-14T10:15:37.798479+08"
        );
        assert_eq!(song.title(), "Some song");
        assert_eq!(song.artist(), "Someone");
        assert_eq!(song.album(), "SomeAlbum");
        assert_eq!(song.release_date().as_deref(), Some("00-00-0000"));

        assert_eq!(song.external_links().len(), 0);

        assert_eq!(song.album_art_link().as_deref(), Some("https://album.png"));
        assert_eq!(song.playback_link().as_deref(), Some("https://test.mp3"));
        assert_eq!(song.lyrics().as_deref(), Some("Some song lyrics"));

        assert!(!song.is_newly_heard());
    }

    #[test]
    fn deserialize_without_song_id() {
        let song_1: Song = serde_json::from_str("{}").unwrap();
        let song_2: Song = serde_json::from_str("{}").unwrap();
        let song_3: Song = serde_json::from_str("{}").unwrap();

        // Make sure that the song id is unique
        // even it is not defined
        assert_ne!(song_1.id(), song_2.id());
        assert_ne!(song_2.id(), song_3.id());
    }
}
