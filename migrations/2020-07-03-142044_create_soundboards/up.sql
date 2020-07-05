CREATE TABLE "soundboards" (
	"id"	INTEGER,
	"name"	TEXT NOT NULL,
    "path"	TEXT NOT NULL,
	"hotkey"	TEXT,
	"position"	INTEGER,
	"disabled"	INTEGER,
	PRIMARY KEY("id")
);