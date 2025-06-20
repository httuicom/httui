package strings

import (
	"fmt"
	"strings"

	"github.com/gandarfh/httui/pkg/truncate"
)

func AddWhiteSpace(value string, size, maxsize int) string {
	if len(value) == 0 {
		value = "-"
	}
	value = truncate.String(value, maxsize)

	s := strings.Repeat(" ", size)
	s = s[len(value):]
	s = fmt.Sprint(value, s)

	return s
}
