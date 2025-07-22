package storage

import (
	"fmt"
	"time"

	"gorm.io/datatypes"
	"gorm.io/gorm"
)

type Response struct {
	gorm.Model
	RequestExternalId string                          `json:"externalRequestId,omitempty"`
	ExternalId        string                          `json:"_id,omitempty"`
	Sync              *bool                           `json:"sync,omitempty"`
	Status            string                          `json:"status"`
	WorkspaceId       uint                            `json:"workspaceId"`
	RequestId         uint                            `json:"requestId"`
	Response          datatypes.JSONType[interface{}] `json:"response"`
	Request           datatypes.JSONType[Request]     `json:"request,omitempty"`
}

func (r Response) GetID() string {
	return fmt.Sprint(r.ID)
}

func (r Response) GetExternalID() string {
	return r.ExternalId
}

func (r Response) GetUpdatedAt() time.Time {
	return r.UpdatedAt
}

type ResponsesRepo struct {
	Sql *gorm.DB
}

func NewResponse(db *gorm.DB) *ResponsesRepo {
	return &ResponsesRepo{db}
}

func (repo *ResponsesRepo) Create(value *Response) error {
	return repo.Sql.Create(value).Error
}

func (repo *ResponsesRepo) FindOne(requestId, workspace_id uint) (*Response, error) {
	request := Response{}
	err := repo.Sql.Model(&request).
		Where("request_id = ?", requestId).
		Where("workspace_id = ?", workspace_id).
		Order("created_at DESC").
		First(&request).Error

	return &request, err
}

func (repo *ResponsesRepo) ListForSync() ([]Response, error) {
	responses := []Response{}

	if err := repo.Sql.Model(&responses).
		Where("sync = ?", 0).
		Find(&responses).Error; err != nil {
		return responses, err
	}

	return responses, nil
}
