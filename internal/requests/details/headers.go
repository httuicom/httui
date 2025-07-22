package details

import (
	"fmt"

	"github.com/charmbracelet/glamour"
	"github.com/charmbracelet/lipgloss"
	"github.com/gandarfh/httui/pkg/utils"
)

func (m Model) Headers() string {
	headerWidth := m.Width/3 - 2
	maxTextValueSize := headerWidth / 4

	paramrenderer, _ := glamour.NewTermRenderer(
		glamour.WithStyles(styleConfig),
		glamour.WithWordWrap(headerWidth),
	)

	rawheader := utils.GetAllParentsHeaders(m.RequestsRepo, m.Request.ParentID, m.Request.Headers.Data())
	jsonString := DataToString(rawheader, maxTextValueSize, m.Height/3-2)

	header, _ := paramrenderer.Render("```json\n" + jsonString + "\n  ```")

	header_box := lipgloss.NewStyle().PaddingTop(1).Height(m.Height/2 - 2).Render(lipgloss.JoinVertical(
		lipgloss.Left,
		lipgloss.NewStyle().Bold(true).Render(fmt.Sprint(" Headers:")),
		header,
	))

	return header_box
}
