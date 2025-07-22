package utils

import (
	"errors"
	"fmt"
	"regexp"
	"strconv"
	"strings"

	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/convert"
)

var re_env = regexp.MustCompile(`{{ _.\w* }}`)
var re_response = regexp.MustCompile(`{% response '\w+(\.\w+)*' '\d*' %}`)
var re_response_fields = regexp.MustCompile(`'\w+(\.\w+)*'`)

// {% response 'field' '1' %}

func ReadEnv(key string, envs map[string]string) string {
	key = strings.Replace(key, "{{ _.", "", 1)
	key = strings.Replace(key, " }}", "", 1)

	for envkey, value := range envs {
		if envkey == key {
			return value
		}
	}

	return key
}

func getField(payload map[string]interface{}, fields []string) (interface{}, error) {
	if len(fields) == 0 {
		return payload, nil
	}

	field := fields[0]
	value, found := payload[field]
	if !found {
		return nil, errors.New("Not found")
	}

	if submap, ok := value.(map[string]interface{}); ok {
		return getField(submap, fields[1:])
	}

	return value, nil
}

func ReadResponse(responsesRepo *storage.ResponsesRepo, key string, workspaceId uint) (string, error) {
	infos := re_response_fields.FindAllString(key, -1)

	infos[0] = strings.Replace(infos[0], "'", "", -1)
	reqId, _ := strconv.Atoi(strings.Replace(infos[1], "'", "", -1))

	fields := strings.Split(infos[0], ".")

	response, err := responsesRepo.FindOne(uint(reqId), workspaceId)

	data := map[string]interface{}{}
	convert.ToSource(response, &data)

	value, _ := getField(data, fields)

	return fmt.Sprint(value), err
}

type TransformFunc func(text string) string

func ReplaceByOperator(responsesRepo *storage.ResponsesRepo, raw string, workspaceId uint, envs map[string]string) string {
	listOfEnvs := re_env.FindAllString(raw, -1)
	listOfResponses := re_response.FindAllString(raw, -1)

	for _, item := range listOfEnvs {
		if value := ReadEnv(item, envs); value == "" {
			raw = strings.ReplaceAll(raw, item, item)
		} else {
			raw = strings.ReplaceAll(raw, item, value)
		}
	}

	for _, item := range listOfResponses {
		if data, err := ReadResponse(responsesRepo, item, workspaceId); err != nil {
			raw = strings.ReplaceAll(raw, item, item)
		} else {
			raw = strings.ReplaceAll(raw, item, data)
		}
	}

	return raw
}
