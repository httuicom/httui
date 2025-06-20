package details

import (
	"fmt"
	"strings"

	"github.com/charmbracelet/lipgloss"
	"github.com/gandarfh/httui/pkg/truncate"
)

func (m Model) Preview() string {
	maxPreviewTextSize := m.Width - m.Width/6
	preview := truncate.String(fmt.Sprintf("%s - %s", strings.ToUpper(string(m.Request.Method)), string(m.Request.Endpoint)), maxPreviewTextSize)

	if m.Cursor == CursorPreview {
		m.InputPreview.Prompt = fmt.Sprintf("%s - ", strings.ToUpper(string(m.Request.Method)))
		m.InputPreview.CharLimit = maxPreviewTextSize
		preview = m.InputPreview.View()
	}

	text := lipgloss.NewStyle().Width(m.Width).Border(lipgloss.RoundedBorder()).Render(
		lipgloss.NewStyle().Bold(true).Render(fmt.Sprint(" Preview: ", preview)),
	)

	return text
}
