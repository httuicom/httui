package storage

import (
	"time"

	"github.com/gandarfh/httui/pkg/tree/v2"
	"gorm.io/datatypes"
	"gorm.io/gorm"
)

type DefaultsRepo struct {
	Sql *gorm.DB
}

type Default struct {
	gorm.Model
	LastResponseSync  time.Time                                `json:"lastResponseSync"`
	LastRequestSync   time.Time                                `json:"lastRequestSync"`
	LastWorkspaceSync time.Time                                `json:"lastWorkspaceSync"`
	WorkspaceId       uint                                     `json:"workspaceId"`
	RequestId         uint                                     `json:"requestId"`
	Cursor            *int                                     `json:"cursor"`
	Page              *int                                     `json:"page"`
	RequestTree       datatypes.JSONType[[]tree.Node[Request]] `json:"requestTree"`
}

func NewDefault(db *gorm.DB) *DefaultsRepo {
	items := []Default{}
	db.Find(&items)

	if len(items) == 0 {
		db.Create(&Default{})
	}

	return &DefaultsRepo{db}
}

func (repo *DefaultsRepo) Update(value Default) error {
	db := repo.Sql.Model(&Default{}).Where("id IS ?", 1).Updates(value)
	return db.Error
}

func (repo *DefaultsRepo) First() (*Default, error) {
	value := Default{}

	db := repo.Sql.Model(&value).First(&value)
	return &value, db.Error
}
