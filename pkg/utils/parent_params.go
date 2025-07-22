package utils

import "github.com/gandarfh/httui/internal/storage"

func CopySliceOfMap[K comparable, V any](src []map[K]V) []map[K]V {
	copied := make([]map[K]V, len(src))
	for i, m := range src {
		newMap := make(map[K]V)
		for k, v := range m {
			newMap[k] = v
		}
		copied[i] = newMap
	}
	return copied
}

func GetAllParentsHeaders(requestsRepo *storage.RequestsRepo, parentId *uint, headers []map[string]string) []map[string]string {
	updatedHeaders := CopySliceOfMap(headers)

	if parentId != nil {
		parent, _ := requestsRepo.FindOne(*parentId)
		parentHeaders := GetAllParentsHeaders(requestsRepo, parent.ParentID, parent.Headers.Data())
		updatedHeaders = append(updatedHeaders, CopySliceOfMap(parentHeaders)...)
	}

	return updatedHeaders
}

func GetAllParentsParams(requestsRepo *storage.RequestsRepo, parentId *uint, params []map[string]string) []map[string]string {
	updatedParams := CopySliceOfMap(params)

	if parentId != nil {
		parent, _ := requestsRepo.FindOne(*parentId)
		parentParams := GetAllParentsParams(requestsRepo, parent.ParentID, parent.QueryParams.Data())
		updatedParams = append(updatedParams, CopySliceOfMap(parentParams)...)
	}

	return updatedParams
}
