package requests

import (
	"log"

	tea "github.com/charmbracelet/bubbletea"
	"github.com/gandarfh/httui/internal/storage"
	"github.com/gandarfh/httui/pkg/tree/v2"
)

func LoadDefault() tea.Msg {
	config, _ := storage.NewDefault().First()
	return *config
}

func LoadWorspace() tea.Msg {
	config, _ := storage.NewDefault().First()
	workspace, _ := storage.NewWorkspace().FindOne(config.WorkspaceId)
	log.Println(workspace.ID, workspace.Name)

	return workspace
}

type RequestsData struct {
	List        []storage.Request
	Current     storage.Request
	RequestTree []tree.Node[storage.Request]
	ParentID    *uint
	Cursor      int
	Page        int
}

func LoadRequests() tea.Msg {
	config, _ := storage.NewDefault().First()
	request, _ := storage.NewRequest().FindOne(config.RequestId)
	requests, _ := storage.NewRequest().List(request.ParentID, "")

	cursor := 0
	page := 0

	if config.Cursor != nil {
		cursor = *config.Cursor
	}

	if config.Page != nil {
		page = *config.Page
	}

	return RequestsData{
		RequestTree: config.RequestTree.Data(),
		Cursor:      cursor,
		Page:        page,
		Current:     *request,
		List:        requests,
		ParentID:    request.ParentID,
	}
}

func LoadRequestsByParentId(parentId *uint) tea.Cmd {
	return func() tea.Msg {
		config, _ := storage.NewDefault().First()
		request, _ := storage.NewRequest().FindOne(config.RequestId)
		requests, _ := storage.NewRequest().List(parentId, "")

		return RequestsData{
			List:        requests,
			ParentID:    parentId,
			RequestTree: config.RequestTree.Data(),
			Cursor:      *config.Cursor,
			Page:        *config.Page,
			Current:     *request,
		}
	}
}

func LoadRequestsByFilter(filter string) tea.Cmd {
	return func() tea.Msg {
		config, _ := storage.NewDefault().First()
		request, _ := storage.NewRequest().FindOne(config.RequestId)
		requests, _ := storage.NewRequest().List(nil, filter)

		return RequestsData{
			RequestTree: config.RequestTree.Data(),
			List:        requests,
			Current:     *request,
			ParentID:    nil,
			Cursor:      0,
			Page:        0,
		}
	}
}
