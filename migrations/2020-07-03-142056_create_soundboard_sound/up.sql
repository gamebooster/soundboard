CREATE TABLE "soundboard_sound" (
	"soundboard_id"	INTEGER NOT NULL CHECK(typeof("soundboard_id") = 'integer'),
	"sound_id"	INTEGER NOT NULL CHECK(typeof("sound_id") = 'integer'),
	PRIMARY KEY("soundboard_id","sound_id"),
	FOREIGN KEY("sound_id") REFERENCES "sounds"("id"),
	FOREIGN KEY("soundboard_id") REFERENCES "soundboards"("id")
)