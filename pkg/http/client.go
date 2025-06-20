package client

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"
)

type Header struct {
	Key   string
	Value string
}

type Params struct {
	Key   string
	Value string
}

type Client struct {
	method  string
	body    io.Reader
	auth    *Header
	url     []string
	params  []*Params
	headers []*Header
	cookies []*http.Cookie
}

type Response struct {
	*http.Response
	Error error
}

func (c Client) URL(url string) Client {
	if strings.HasPrefix(url, "/") {
		url = strings.TrimPrefix(url, "/")
	}

	if strings.HasSuffix(url, "/") {
		url = strings.TrimSuffix(url, "/")
	}

	c.url = append(c.url, url)

	return c
}

func (c Client) Auth(key string, value string) Client {
	c.auth = &Header{
		Key:   key,
		Value: value,
	}

	return c
}

func (c Client) Header(key string, value string) Client {
	c.headers = append(c.headers, &Header{
		Key:   key,
		Value: value,
	})
	return c
}

func (c Client) Cookie(cookie *http.Cookie) Client {
	c.cookies = append(c.cookies, cookie)
	return c
}

func (c Client) Params(key string, value string) Client {
	c.params = append(c.params, &Params{Key: key, Value: value})
	return c
}

func (c Client) Body(b []byte) Client {
	c.body = bytes.NewBuffer(b)
	return c
}

func (c Client) Do() (*Response, error) {
	url := strings.Join(c.url, "/")

	request, err := http.NewRequest(c.method, url, c.body)
	if err != nil {
		return nil, err
	}

	q := request.URL.Query()
	for _, item := range c.params {
		q.Add(item.Key, item.Value)
	}
	request.URL.RawQuery = q.Encode()

	request.Header.Add("accept", "application/json")
	request.Header.Add("Content-Type", "application/json; charset=utf-8")

	if c.auth != nil {
		request.Header.Add(c.auth.Key, c.auth.Value)
	}

	for _, item := range c.headers {
		request.Header.Add(item.Key, item.Value)
	}

	SERVER_READ_TIMEOUT, _ := strconv.Atoi(os.Getenv("SERVER_READ_TIMEOUT"))
	client := http.Client{
		Timeout: time.Second * time.Duration(SERVER_READ_TIMEOUT),
	}

	response, err := client.Do(request)
	if err != nil {
		return nil, err
	}

	return &Response{Response: response}, nil
}

func (r *Response) Decode(decode any) error {
	data, err := io.ReadAll(r.Body)
	if err != nil {
		return err
	}

	if r.StatusCode >= 400 {
		var response any
		if err := json.Unmarshal(data, &response); err != nil {
			return err
		}
		return fmt.Errorf("%v", response)
	}

	if decode != nil {
		if err := json.Unmarshal(data, decode); err != nil {
			return err
		}

	}

	return nil
}

func New() Client {
	return Client{}
}

func (c Client) Get(endpoint string) (*Response, error) {
	c.method = http.MethodGet
	return c.URL(endpoint).Do()
}

func (c Client) Post(endpoint string) (*Response, error) {
	c.method = http.MethodPost
	return c.URL(endpoint).Do()
}

func (c Client) Patch(endpoint string) (*Response, error) {
	c.method = http.MethodPatch
	return c.URL(endpoint).Do()
}

func (c Client) Delete(endpoint string) (*Response, error) {
	c.method = http.MethodDelete
	return c.URL(endpoint).Do()
}

func (c Client) Put(endpoint string) (*Response, error) {
	c.method = http.MethodPut
	return c.URL(endpoint).Do()
}
