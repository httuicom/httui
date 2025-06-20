package utils

import "reflect"

func HasZeroValue(v interface{}) bool {
	val := reflect.ValueOf(v)
	for i := 0; i < val.NumField(); i++ {
		field := val.Field(i)
		if field.IsZero() {
			return true
		}
	}

	return false
}
