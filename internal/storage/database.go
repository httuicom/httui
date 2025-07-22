package storage

import (
	"log"
	"os"
	"path/filepath"
	"time"

	"gorm.io/driver/sqlite"
	"gorm.io/gorm"
	"gorm.io/gorm/logger"
)

type Entity interface {
	GetID() string
	GetExternalID() string
	GetUpdatedAt() time.Time
}

var newLogger = logger.New(
	log.Default(),
	logger.Config{
		// LogLevel: logger.Error,
	},
)

func SqliteConnection() (*gorm.DB, error) {
	home, _ := os.UserHomeDir()
	db, err := gorm.Open(sqlite.Open(filepath.Join(home, "httui.v3.db")), &gorm.Config{
		Logger: newLogger,
		NowFunc: func() time.Time {
			return time.Now().UTC()
		},
	})

	if err != nil {
		return nil, err
	}

	sqldb, _ := db.DB()

	if err := sqldb.Ping(); err != nil {
		defer sqldb.Close()
		return nil, err
	}

	db.AutoMigrate(&Default{})
	db.AutoMigrate(&Request{}, &Response{})
	db.AutoMigrate(&Workspace{})

	return db, nil
}
