package utils

func ProcessParamsOperators(values []map[string]string, workspaceId uint, envs map[string]string) []map[string]string {
	updatedEnvs := CopySliceOfMap(values)

	for i, item := range values {
		for k, v := range item {
			updatedEnvs[i][k] = ReplaceByOperator(v, workspaceId, envs)
		}
	}

	return updatedEnvs
}
