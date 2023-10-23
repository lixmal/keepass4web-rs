import React from 'react'
import {Link} from "react-router-dom"


import NavBar from "./NavBar"
import Alert from "./Alert"
import withNavigateHook from "./nagivateHook";

class CallbackUserAuth extends React.Component {
    constructor(props) {
        super(props)
        this.state = {
            error: null,
        }
    }

    componentDidMount() {
        const {navigate} = this.props

        let kp = window.KeePass4WebResponse
        let err
        if (kp) {
            if (kp.success && kp.data) {
                KeePass4Web.setCSRFToken(kp.data.csrf_token)
                KeePass4Web.setSettings(kp.data.settings)

                return setTimeout(function () {
                    navigate('/', {replace: true})
                }, 0)
            } else {
                err = kp.message
            }
        } else {
            err = "Failed to retrieve session data"
        }

        this.setState({
            error: err,
        })
    }

    render() {
        return (
            <div>
                <NavBar/>
                <div className="container">
                    <div className="kp-login">
                        <Alert error={this.state.error}/>
                        <Link to="/" replace>Get me home</Link>
                    </div>
                </div>
            </div>
        )
    }
}

export default withNavigateHook(CallbackUserAuth)
