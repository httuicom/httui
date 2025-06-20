package details

import (
	"encoding/json"
	"strings"

	"github.com/gandarfh/httui/pkg/truncate"
)

func DataToString(data interface{}, elementSize, elementsCount int) string {
	indentedJSON, err := json.MarshalIndent(data, "", "\t")
	if err != nil {
		return ""
	}

	// dataWithEnvValues := utils.ReplaceByOperator(string(indentedJSON), workspaceId)
	dataWithEnvValues := string(indentedJSON)

	summariseProperties := func(s string) string {
		// s = utils.ReplaceByOperator(s, workspaceId)
		return truncate.String(s, elementSize)
	}

	value, _ := replaceStringsInJSON(
		dataWithEnvValues,
		summariseProperties,
		elementsCount,
	)

	return value
}

func modifyData(value interface{}, replaceFunc func(string) string, size int) interface{} {
	// maxSliceElements := size / 6
	// maxMapElements := size / 6

	switch v := value.(type) {
	case map[string]interface{}:
		// if len(v) >= maxMapElements {
		// 	v = limitMapSize(v, maxMapElements)
		// }
		return processMap(v, replaceFunc, size)
	case []interface{}:
		// if len(v) > maxSliceElements {
		// 	v = v[:maxSliceElements]
		// }
		return processSlice(v, replaceFunc, size)
	case string:
		return replaceFunc(v)
	default:
		return v
	}
}

func replaceStringsInJSON(input string, replaceFunc func(string) string, maxLines int) (string, error) {
	var data interface{}
	err := json.Unmarshal([]byte(input), &data)
	if err != nil {
		return "", err
	}

	data = modifyData(data, replaceFunc, maxLines)

	result, err := json.MarshalIndent(data, "", "  ")
	if err != nil {
		return "", err
	}

	if maxLines > 0 {
		lines := strings.Split(string(result), "\n")
		if len(lines) > maxLines {
			lines[maxLines-1] = "// ..."
			return strings.Join(lines[:maxLines], "\n"), nil
		}
	}

	return string(result), nil
}

func limitMapSize(m map[string]interface{}, max int) map[string]interface{} {
	newMap := make(map[string]interface{})
	count := 0
	for key, value := range m {
		if count >= max {
			break
		}
		newMap[key] = value
		count++
	}
	return newMap
}

func processMap(m map[string]interface{}, replaceFunc func(string) string, size int) map[string]interface{} {
	for key, val := range m {
		m[key] = modifyData(val, replaceFunc, size)
	}
	return m
}

func processSlice(s []interface{}, replaceFunc func(string) string, size int) []interface{} {
	for i, val := range s {
		s[i] = modifyData(val, replaceFunc, size)
	}
	return s
}
