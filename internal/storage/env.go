package storage

import (
	"gorm.io/gorm"
)

type Env struct {
	WorkspaceId string `json:"workspaceId"`
	Key         string `json:"key"`
	Value       string `json:"value"`
}

type EnvsRepo struct {
	Sql *gorm.DB
}

func NewEnvs() *EnvsRepo {
	db := Database

	return &EnvsRepo{db}
}

func (repo *EnvsRepo) Create(env *Env) error {
	db := repo.Sql.Create(env)

	return db.Error
}

func (repo *EnvsRepo) Update(value *Env) error {
	return repo.Sql.Model(value).
		Save(value).Error
}

func (repo *EnvsRepo) Delete(id string) error {
	db := repo.Sql.Model(&Env{})
	db.Where("id IS ?", id).Delete(&Env{})

	return db.Error
}

func (repo *EnvsRepo) Find(id string) (Env, error) {
	value := Env{}
	db := repo.Sql.Model(&Env{})
	db.First(&value, id)

	return value, db.Error
}

func (repo *EnvsRepo) FindByKey(key string, workspaceId string) (Env, error) {
	value := Env{Key: key}
	db := repo.Sql.Model(&Env{})
	db.Where("key = ?", key).
		Where("workspace_id = ?", workspaceId).
		FirstOrCreate(&value)

	return value, db.Error
}

func (repo *EnvsRepo) List(workspaceId string) ([]Env, error) {
	envs := []Env{}
	db := repo.Sql.Where("workspace_id = ?", workspaceId).Find(&envs)

	return envs, db.Error
}
