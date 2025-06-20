package storage

import (
	"database/sql/driver"
	"fmt"
	"strings"
	"time"

	"gorm.io/datatypes"
	"gorm.io/gorm"
)

type Time time.Time

func (t *Time) Scan(value interface{}) error {
	switch v := value.(type) {
	case []byte:
		return t.UnmarshalText(string(v))
	case string:
		return t.UnmarshalText(v)
	case time.Time:
		*t = Time(v)
	case nil:
		*t = Time{}
	default:
		return fmt.Errorf("cannot sql.Scan() MyTime from: %#v", v)
	}
	return nil
}

func (t Time) Value() (driver.Value, error) {
	return driver.Value(time.Time(t).Format(time.RFC3339)), nil
}

func (t *Time) UnmarshalText(value string) error {
	dd, err := time.Parse(time.RFC3339, value)
	if err != nil {
		return err
	}
	*t = Time(dd)
	return nil
}

func (Time) GormDataType() string {
	return "TIME"
}

type Workspace struct {
	gorm.Model
	ExternalId   string                                `json:"_id"`
	Name         string                                `gorm:"unique" json:"name"`
	Sync         *bool                                 `json:"sync"`
	Environments datatypes.JSONType[map[string]string] `json:"environments"`
}

func (w Workspace) GetID() string {
	return fmt.Sprint(w.ID)
}

func (w Workspace) GetExternalID() string {
	return w.ExternalId
}

func (w Workspace) GetUpdatedAt() time.Time {
	return w.UpdatedAt
}

type WorkspacesRepo struct {
	Sql *gorm.DB
}

func NewWorkspace() *WorkspacesRepo {
	db := Database

	return &WorkspacesRepo{db}
}

func (repo *WorkspacesRepo) Create(value *Workspace) error {
	sync := false
	value.Sync = &sync
	value.Name = strings.Trim(value.Name, " ")

	if value.Name == "" {
		return nil
	}
	db := repo.Sql.Create(value)
	return db.Error
}

func (repo *WorkspacesRepo) Update(value *Workspace) error {
	sync := false
	value.Sync = &sync
	if err := repo.Sql.
		Session(&gorm.Session{FullSaveAssociations: true}).
		Where("id = ?", value.ID).
		Updates(value).Error; err != nil {
		return err
	}

	return nil
}

func (repo *WorkspacesRepo) FindOne(id uint) (Workspace, error) {
	workspace := Workspace{}

	db := repo.Sql.Model(&workspace).
		Where("id IS ?", id).First(&workspace)

	if db.Error != nil {
		repo.Sql.Model(&workspace).First(&workspace)
	}

	return workspace, nil
}

func (repo *WorkspacesRepo) List() ([]Workspace, error) {
	workspaces := []Workspace{}

	if err := repo.Sql.Model(&workspaces).
		Find(&workspaces).Error; err != nil {
		return workspaces, err
	}

	return workspaces, nil
}

func (repo *WorkspacesRepo) ListForSync() ([]Workspace, error) {
	workspaces := []Workspace{}

	if err := repo.Sql.Model(&workspaces).
		Where("sync = ?", 0).Or("sync IS NULL").
		Find(&workspaces).Error; err != nil {
		return workspaces, err
	}

	return workspaces, nil
}

func (repo *WorkspacesRepo) Delete(id uint) error {
	db := repo.Sql.Model(&Workspace{}).Where("id IS ?", id).Delete(&Workspace{})
	return db.Error
}

func (repo *WorkspacesRepo) Environments(workspaceId uint) (map[string]string, error) {
	mergedEnvironments := make(map[string]string)

	var workspaces []Workspace
	if err := repo.Sql.Find(&workspaces).Error; err != nil {
		return nil, err
	}

	for _, workspace := range workspaces {
		if workspace.Environments.Data() != nil {
			for key, value := range workspace.Environments.Data() {
				if workspace.ID == workspaceId {
					mergedEnvironments[key] = value
				} else {
					if _, exists := mergedEnvironments[key]; !exists {
						mergedEnvironments[key] = ""
					}
				}
			}
		}
	}

	return mergedEnvironments, nil
}
