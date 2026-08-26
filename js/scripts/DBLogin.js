import React from 'react'
import LoginForm from './LoginForm'
import NavBar from './NavBar'
import Alert from './Alert'
import Info from './Info'
import withNavigateHook from './nagivateHook'
import { IconDatabase, IconKey, IconUpload } from './Icons'

class DBLogin extends LoginForm {
    constructor() {
        super()
        this.url = 'db_login'
        this.handleFile = this.handleFile.bind(this)
        this.state = { ...this.state, fileName: null }
    }

    handleFile(event) {
        const file = event.target.files[0]
        if (!file) return
        this.setState({ fileName: file.name })
        const reader = new FileReader()
        const me = this
        reader.onload = function () {
            me.refs.key.value = reader.result.split(',')[1]
        }
        reader.readAsDataURL(file)
    }

    render() {
        const { fileName } = this.state
        return (
            <div>
                <NavBar/>
                <div className="container">
                    <div className={this.classes()}>
                        <form className="kp-login-inner" onSubmit={this.handleLogin}>
                            <div className="kp-login-icon">
                                <IconDatabase size={28}/>
                            </div>
                            <h4>Open Vault</h4>
                            <p className="kp-login-sub">Enter your credentials to unlock</p>

                            <div className="kp-login-field">
                                <label htmlFor="kp-master-pw">
                                    <IconKey size={13}/>
                                    Master Password
                                </label>
                                <input
                                    id="kp-master-pw"
                                    className="kp-input"
                                    type="password"
                                    ref="password"
                                    placeholder="Enter master password"
                                    autoFocus
                                />
                            </div>

                            <div className="kp-login-field">
                                <label htmlFor="kp-keyfile-input">
                                    <IconUpload size={13}/>
                                    Key File
                                    <span className="kp-login-optional">optional</span>
                                </label>
                                <input
                                    id="kp-keyfile-input"
                                    type="file"
                                    accept="*/*"
                                    ref="keyfile"
                                    onChange={this.handleFile}
                                    style={{ display: 'none' }}
                                />
                                <label htmlFor="kp-keyfile-input" className={`kp-file-label${fileName ? ' has-file' : ''}`}>
                                    <IconUpload size={13}/>
                                    {fileName || 'Choose key file…'}
                                </label>
                            </div>

                            <input id="key" ref="key" type="hidden"/>

                            <button className="kp-btn kp-btn-primary kp-btn-open" type="submit">
                                Open Vault
                            </button>

                            <Alert error={this.state.error}/>
                            <Info info={this.props.location.state && this.props.location.state.info}/>
                        </form>
                    </div>
                </div>
            </div>
        )
    }
}

export default withNavigateHook(DBLogin)
