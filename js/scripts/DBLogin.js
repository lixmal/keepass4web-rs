import React from 'react'
import LoginForm from './LoginForm'
import NavBar from './NavBar'
import Alert from './Alert'
import Info from './Info'
import withNavigateHook from './nagivateHook'

class DBLogin extends LoginForm {
    constructor() {
        super()
        this.url = 'db_login'
        this.handleFile = this.handleFile.bind(this)
    }

    handleFile(event) {
        const file = event.target.files[0]
        const reader = new FileReader()

        const me = this
        reader.onload = function () {
            // race condition!?
            me.refs.key.value = reader.result.split(',')[1]
        }
        reader.readAsDataURL(file)
    }

    render() {
        return (
            <div>
                <NavBar/>
                <div className="container">
                    <div className={this.classes()}>
                        <form className="kp-login-inner" onSubmit={this.handleLogin}>
                            <h4>KeePass Login</h4>
                            <input className="form-control user" type="password" ref="password"
                                   placeholder="Master Password" autoFocus="autoFocus"/>
                            <input className="input-group btn" type="file" accept="*/*" ref="keyfile"
                                   placeholder="Key file" onChange={this.handleFile}/>
                            <input id="key" ref="key" type="hidden"/>
                            <button className="btn btn-block btn-lg btn-success" type="submit">Open</button>
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
