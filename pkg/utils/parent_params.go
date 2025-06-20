package utils

import "github.com/gandarfh/httui/internal/storage"

func GetAllParentsHeaders(parentId *uint, headers []map[string]string) []map[string]string {
	if parentId != nil {
		parent, _ := storage.NewRequest().FindOne(*parentId)
		parentHeaders := GetAllParentsHeaders(parent.ParentID, parent.Headers.Data())

		headers = append(headers, parentHeaders...)
	}

	return headers
}

func GetAllParentsParams(parentId *uint, params []map[string]string) []map[string]string {
	if parentId != nil {
		parent, _ := storage.NewRequest().FindOne(*parentId)
		parentParams := GetAllParentsParams(parent.ParentID, parent.QueryParams.Data())

		params = append(params, parentParams...)
	}

	return params
}
