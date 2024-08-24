-- Add migration script here
CREATE TABLE "comments" (
	"id"	INTEGER,
	"author"	TEXT,
	"body"	TEXT,
	"post_id"	INTEGER
, "parent_id"	INTEGER)

CREATE TABLE "posts" (
	"id"	INTEGER,
	"title"	TEXT,
	"body"	TEXT,
	"author"	TEXT
)
