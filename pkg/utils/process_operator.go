package utils

import "github.com/gandarfh/httui/internal/storage"

func ProcessParamsOperators(responsesRepo *storage.ResponsesRepo, values []map[string]string, workspaceId uint, envs map[string]string) []map[string]string {
	updatedEnvs := CopySliceOfMap(values)

	for i, item := range values {
		for k, v := range item {
			updatedEnvs[i][k] = ReplaceByOperator(responsesRepo, v, workspaceId, envs)
		}
	}

	return updatedEnvs
}
