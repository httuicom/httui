package storage

import (
	"fmt"
	"strings"
	"time"

	"gorm.io/datatypes"
	"gorm.io/gorm"
)

type RequestType string

const (
	REQUEST RequestType = "request"
	GROUP   RequestType = "group"
)

type MethodType string

const (
	GET    MethodType = "get"
	POST   MethodType = "post"
	PATCH  MethodType = "patch"
	PUT    MethodType = "put"
	DELETE MethodType = "delete"
)

type Request struct {
	gorm.Model
	ExternalId     string                                  `json:"_id,omitempty"`
	Sync           *bool                                   `json:"sync,omitempty"`
	OrganizationID string                                  `gorm:"index;" json:"organizationId,omitempty"`
	ParentID       *uint                                   `gorm:"index;" json:"parentId,omitempty"`
	Type           RequestType                             `json:"type"`
	Name           string                                  `json:"name"`
	Description    *string                                 `json:"description"`
	Method         MethodType                              `json:"method,omitempty"`
	Endpoint       string                                  `json:"endpoint"`
	QueryParams    datatypes.JSONType[[]map[string]string] `json:"queryParams"`
	Headers        datatypes.JSONType[[]map[string]string] `json:"headers"`
	Body           datatypes.JSONType[map[string]any]      `json:"body"`
}

func (r Request) GetID() string {
	return fmt.Sprint(r.ID)
}

func (r Request) GetExternalID() string {
	return r.ExternalId
}

func (r Request) GetUpdatedAt() time.Time {
	return r.UpdatedAt
}

type RequestsRepo struct {
	Sql *gorm.DB
}

func NewRequest() *RequestsRepo {
	db := Database

	return &RequestsRepo{db}
}

func (repo *RequestsRepo) Create(value *Request) error {
	sync := false
	value.Sync = &sync

	value.Method = MethodType(strings.ToLower(string(value.Method)))

	if err := repo.Sql.Create(value).Error; err != nil {
		return err
	}

	return nil
}

func (repo *RequestsRepo) Update(value *Request) error {
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

func (repo *RequestsRepo) Upsert(value *Request) error {
	if err := repo.Sql.
		Session(&gorm.Session{FullSaveAssociations: true}).
		Where("id = ?", value.ID).
		Updates(value).Error; err != nil {
		return err
	}

	return nil
}

func (repo *RequestsRepo) FindOne(id uint) (*Request, error) {
	request := Request{}

	if err := repo.Sql.Model(&request).Where("id = ?", id).First(&request).Error; err != nil {
		return &request, err
	}

	return &request, nil
}

func (repo *RequestsRepo) List(parentId *uint, filter string) ([]Request, error) {
	var requests []Request
	query := repo.Sql.Model(&requests)

	if filter != "" {
		query.Where("name LIKE ?", "%"+filter+"%")

		if err := query.Find(&requests).Error; err != nil {
			return nil, err
		}

		if len(requests) == 0 {
			return requests, nil
		}

		relatedRequestsMap := make(map[uint]Request)

		for _, req := range requests {
			relatedRequestsMap[req.ID] = req
		}

		addRelatedRequests := func(req Request) {
			if req.ParentID != nil {
				var parent Request
				if err := repo.Sql.Where("id = ?", req.ParentID).First(&parent).Error; err == nil {
					relatedRequestsMap[parent.ID] = parent
				}
			}

			if req.Type == "group" {
				var children []Request
				if err := repo.Sql.Where("parent_id = ?", req.ID).Find(&children).Error; err == nil {
					for _, child := range children {
						relatedRequestsMap[child.ID] = child
					}
				}
			}
		}

		for _, req := range requests {
			if req.Type == "request" || req.Type == "group" {
				addRelatedRequests(req)
			}
		}

		finalRequests := make([]Request, 0, len(relatedRequestsMap))
		for _, req := range relatedRequestsMap {
			finalRequests = append(finalRequests, req)
		}

		return finalRequests, nil
	}

	if err := query.Find(&requests).Error; err != nil {
		return nil, err
	}

	return requests, nil
}

func (repo *RequestsRepo) ListByparent(parentId *uint) ([]Request, error) {
	requests := []Request{}
	query := repo.Sql.Model(&requests)

	if parentId == nil {
		query = query.Where("parent_id IS NULL")
	} else {
		query = query.Where("parent_id = ?", *parentId)
	}

	if err := query.Find(&requests).Error; err != nil {
		return requests, err
	}

	return requests, nil
}

func (repo *RequestsRepo) ListForSync() ([]Request, error) {
	requests := []Request{}

	if err := repo.Sql.Model(&requests).
		Where("sync = ?", 0).Or("sync IS NULL").
		Find(&requests).Error; err != nil {
		return requests, err
	}

	return requests, nil
}

func (repo *RequestsRepo) Delete(id uint) error {
	if err := repo.Sql.Model(&Request{}).
		Where("id = ?", id).
		Delete(&Request{}).Error; err != nil {
		return err
	}
	return nil
}
